mod navigation;
mod notifications;
mod servers;
mod workspaces;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use notify_rust::{Notification, NotificationResponse, Timeout};
use serde::Serialize;
use tauri::ipc::CapabilityBuilder;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::webview::{DownloadEvent, NewWindowFeatures, NewWindowResponse, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, RunEvent, Runtime, State, Webview,
    WebviewUrl, WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_window_state::StateFlags;
use url::Url;

use servers::{Server, Store};
use workspaces::Workspace;

const MAIN: &str = "main";
const RAIL: &str = "rail";
const PICKER: &str = "picker";
const TRAY: &str = "tray";
const STARTED_HIDDEN: &str = "--hidden";
const RAIL_WIDTH: f64 = 72.0;
const NOTIFICATION_LINGERS_MS: u32 = 60 * 60 * 1000;

const BRIDGE: &str = include_str!("bridge.js");

static VIEWERS: AtomicUsize = AtomicUsize::new(0);
static GRANTS: AtomicUsize = AtomicUsize::new(0);

type Servers = Mutex<Store>;

#[derive(Default)]
struct Showing(Mutex<String>);

#[derive(Clone, Default)]
struct Report {
    workspaces: Vec<Workspace>,
    current: Option<String>,
}

#[derive(Default)]
struct Reports(Mutex<HashMap<String, Report>>);

#[derive(Clone, Serialize)]
struct ServerView {
    #[serde(flatten)]
    server: Server,
    workspaces: Vec<Workspace>,
    current: Option<String>,
}

#[derive(Clone, Serialize)]
struct Overview {
    servers: Vec<ServerView>,
    active: Option<String>,
}

fn webview_label(url: &str) -> String {
    let safe: String = url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("server-{safe}")
}

fn overview<R: Runtime>(app: &AppHandle<R>) -> Overview {
    let servers = app
        .state::<Servers>()
        .lock()
        .map(|store| store.servers().to_vec())
        .unwrap_or_default();
    let showing = app
        .state::<Showing>()
        .0
        .lock()
        .map(|label| label.clone())
        .unwrap_or_default();
    let active = servers
        .iter()
        .find(|s| webview_label(&s.url) == showing)
        .map(|s| s.url.clone());
    let reports = app
        .state::<Reports>()
        .0
        .lock()
        .map(|r| r.clone())
        .unwrap_or_default();
    let servers = servers
        .into_iter()
        .map(|server| {
            let report = reports.get(&server.url).cloned().unwrap_or_default();
            let workspaces = report
                .workspaces
                .into_iter()
                .filter(|w| !server.hidden.contains(&w.id))
                .collect();
            ServerView {
                server,
                workspaces,
                current: report.current,
            }
        })
        .collect();
    Overview { servers, active }
}

fn badge<R: Runtime>(app: &AppHandle<R>, servers: &[ServerView]) {
    let unread = servers
        .iter()
        .flat_map(|s| &s.workspaces)
        .map(|w| w.unread)
        .fold(0, u32::saturating_add);
    if let Ok(window) = main_window(app) {
        let _ = window.set_badge_count((unread > 0).then_some(i64::from(unread)));
    }
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_title((unread > 0).then(|| unread.to_string()));
        let _ = tray.set_tooltip(Some(match unread {
            0 => "Chat Systems".to_owned(),
            1 => "Chat Systems: 1 unread".to_owned(),
            n => format!("Chat Systems: {n} unread"),
        }));
    }
}

fn announce<R: Runtime>(app: &AppHandle<R>) {
    let now = overview(app);
    badge(app, &now.servers);
    let _ = app.emit_to(RAIL, "servers-changed", now.clone());
    let _ = app.emit_to(PICKER, "servers-changed", now);
    if let Ok(menu) = build_menu(app) {
        let _ = app.set_menu(menu);
    }
}

fn open_externally<R: Runtime>(app: &AppHandle<R>, target: &Url) {
    if matches!(target.scheme(), "https" | "http" | "mailto") {
        let _ = app.opener().open_url(target.as_str(), None::<&str>);
    }
}

fn stay_on<R: Runtime>(app: &AppHandle<R>, origin: Url) -> impl Fn(&Url) -> bool + Send + 'static {
    let app = app.clone();
    move |target| {
        if navigation::same_origin(&origin, target) {
            return true;
        }
        open_externally(&app, target);
        false
    }
}

fn save_downloads<R: Runtime>(webview: Webview<R>, event: DownloadEvent<'_>) -> bool {
    if let DownloadEvent::Requested { url, destination } = event {
        if let Ok(dir) = webview.path().download_dir() {
            *destination = navigation::download_path(&dir, destination, &url);
        }
    }
    true
}

fn popups<R: Runtime>(
    app: &AppHandle<R>,
    origin: Url,
) -> impl Fn(Url, NewWindowFeatures) -> NewWindowResponse<R> + Send + 'static {
    let app = app.clone();
    move |target, features| {
        if !navigation::same_origin(&origin, &target) {
            open_externally(&app, &target);
            return NewWindowResponse::Deny;
        }
        let label = format!("viewer-{}", VIEWERS.fetch_add(1, Ordering::Relaxed));
        let viewer = WebviewWindowBuilder::new(&app, label, WebviewUrl::External(target.clone()))
            .window_features(features)
            .title(target.as_str())
            .on_document_title_changed(|window, title| {
                let _ = window.set_title(&title);
            })
            .on_navigation(stay_on(&app, origin.clone()))
            .on_new_window(Box::new(popups(&app, origin.clone()))
                as Box<dyn Fn(Url, NewWindowFeatures) -> NewWindowResponse<R> + Send>)
            .on_download(save_downloads);
        match viewer.build() {
            Ok(window) => NewWindowResponse::Create { window },
            Err(_) => NewWindowResponse::Deny,
        }
    }
}

fn main_window<R: Runtime>(app: &AppHandle<R>) -> Result<Window<R>, String> {
    app.get_window(MAIN)
        .ok_or_else(|| "The main window is gone".to_owned())
}

fn content_bounds<R: Runtime>(window: &Window<R>) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let size = window
        .inner_size()
        .map(|s| s.to_logical::<f64>(scale))
        .unwrap_or(LogicalSize::new(1280.0, 860.0));
    (
        LogicalPosition::new(RAIL_WIDTH, 0.0),
        LogicalSize::new((size.width - RAIL_WIDTH).max(0.0), size.height),
    )
}

fn layout<R: Runtime>(window: &Window<R>) {
    let (position, size) = content_bounds(window);
    for webview in window.webviews() {
        if webview.label() == RAIL {
            let _ = webview.set_position(LogicalPosition::new(0.0, 0.0));
            let _ = webview.set_size(LogicalSize::new(RAIL_WIDTH, size.height));
        } else {
            let _ = webview.set_position(position);
            let _ = webview.set_size(size);
        }
    }
}

fn show_content<R: Runtime>(window: &Window<R>, label: &str) {
    if let Ok(mut showing) = window.state::<Showing>().0.lock() {
        label.clone_into(&mut showing);
    }
    for webview in window.webviews() {
        if webview.label() == RAIL {
            continue;
        }
        if webview.label() == label {
            let _ = webview.show();
            let _ = webview.set_focus();
        } else {
            let _ = webview.hide();
        }
    }
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn load_server<R: Runtime>(
    app: &AppHandle<R>,
    window: &Window<R>,
    server: &Server,
) -> Result<String, String> {
    let label = webview_label(&server.url);
    if window.webviews().iter().all(|w| w.label() != label) {
        let origin = Url::parse(&server.url).map_err(|e| e.to_string())?;
        let _ = app.add_capability(
            CapabilityBuilder::new(format!("server-{}", GRANTS.fetch_add(1, Ordering::Relaxed)))
                .local(false)
                .remote(format!("{}/*", server.url))
                .webview(&label)
                .permission("allow-report-workspaces")
                .permission("allow-notify"),
        );
        let builder = WebviewBuilder::new(&label, WebviewUrl::External(origin.clone()))
            .initialization_script(BRIDGE)
            .background_throttling(BackgroundThrottlingPolicy::Disabled)
            .on_navigation(stay_on(app, origin.clone()))
            .on_new_window(popups(app, origin))
            .on_download(save_downloads);
        let (position, size) = content_bounds(window);
        let webview = window
            .add_child(builder, position, size)
            .map_err(|e| e.to_string())?;
        let _ = webview.hide();
    }
    Ok(label)
}

fn open_server<R: Runtime>(app: &AppHandle<R>, server: &Server) -> Result<(), String> {
    let window = main_window(app)?;
    let label = load_server(app, &window, server)?;
    show_content(&window, &label);
    let _ = window.set_title(&server.name);
    app.state::<Servers>()
        .lock()
        .map_err(|e| e.to_string())?
        .set_last(&server.url)?;
    announce(app);
    Ok(())
}

fn open_picker<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = main_window(app)?;
    if window.webviews().iter().all(|w| w.label() != PICKER) {
        let (position, size) = content_bounds(&window);
        window
            .add_child(
                WebviewBuilder::new(PICKER, WebviewUrl::App("index.html".into())),
                position,
                size,
            )
            .map_err(|e| e.to_string())?;
    }
    show_content(&window, PICKER);
    let _ = window.set_title("Chat Systems");
    announce(app);
    Ok(())
}

fn load_all<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = main_window(app)?;
    let servers = app
        .state::<Servers>()
        .lock()
        .map_err(|e| e.to_string())?
        .servers()
        .to_vec();
    for server in &servers {
        load_server(app, &window, server)?;
    }
    Ok(())
}

fn open_last<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let last = app
        .state::<Servers>()
        .lock()
        .map_err(|e| e.to_string())?
        .last()
        .cloned();
    match last {
        Some(server) => open_server(app, &server),
        None => open_picker(app),
    }
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::default(app)?;
    let servers = app
        .state::<Servers>()
        .lock()
        .map(|store| store.servers().to_vec())
        .unwrap_or_default();

    let submenu = Submenu::new(app, "Servers", true)?;
    for (i, server) in servers.iter().enumerate() {
        let accelerator = (i < 9).then(|| format!("CmdOrCtrl+{}", i + 1));
        submenu.append(&MenuItem::with_id(
            app,
            format!("server:{}", server.url),
            &server.name,
            true,
            accelerator.as_deref(),
        )?)?;
    }
    if !servers.is_empty() {
        submenu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    submenu.append(&MenuItem::with_id(
        app,
        "add-server",
        "Add Server…",
        true,
        Some("CmdOrCtrl+Shift+N"),
    )?)?;
    menu.insert(&submenu, 2)?;
    Ok(menu)
}

fn find<R: Runtime>(app: &AppHandle<R>, url: &str) -> Result<Server, String> {
    app.state::<Servers>()
        .lock()
        .map_err(|e| e.to_string())?
        .find(url)
        .cloned()
        .ok_or_else(|| format!("{url} is not one of your servers"))
}

#[tauri::command]
fn list_servers(app: AppHandle) -> Overview {
    overview(&app)
}

#[tauri::command]
async fn add_server(
    app: AppHandle,
    servers: State<'_, Servers>,
    address: String,
) -> Result<Server, String> {
    let origin = servers::normalize(&address)?;
    let server = servers::probe(&origin).await?;
    servers
        .lock()
        .map_err(|e| e.to_string())?
        .upsert(server.clone())?;
    open_server(&app, &server)?;
    Ok(server)
}

fn forget_server<R: Runtime>(app: &AppHandle<R>, url: &str) -> Result<(), String> {
    app.state::<Servers>()
        .lock()
        .map_err(|e| e.to_string())?
        .remove(url)?;
    let label = webview_label(url);
    let was_showing = app
        .state::<Showing>()
        .0
        .lock()
        .map(|showing| *showing == label)
        .unwrap_or(false);
    if let Some(webview) = app.get_webview(&label) {
        let _ = webview.close();
    }
    if let Ok(mut reports) = app.state::<Reports>().0.lock() {
        reports.remove(url);
    }
    if was_showing {
        return open_last(app);
    }
    announce(app);
    Ok(())
}

#[tauri::command]
fn remove_server(app: AppHandle, url: String) -> Result<(), String> {
    forget_server(&app, &url)
}

fn confirm_remove<R: Runtime>(app: &AppHandle<R>, url: &str) -> Result<(), String> {
    let server = find(app, url)?;
    let answer = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Warning)
        .set_title(format!("Remove {}?", server.name))
        .set_description(format!(
            "{url} leaves the sidebar. You can add it again with its address."
        ))
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            "Remove".into(),
            "Cancel".into(),
        ))
        .show();
    match answer {
        rfd::MessageDialogResult::Ok => forget_server(app, url),
        rfd::MessageDialogResult::Custom(label) if label == "Remove" => forget_server(app, url),
        _ => Ok(()),
    }
}

fn change_hidden<R: Runtime>(
    app: &AppHandle<R>,
    change: impl FnOnce(&mut Store) -> Result<(), String>,
) -> Result<(), String> {
    change(&mut *app.state::<Servers>().lock().map_err(|e| e.to_string())?)?;
    announce(app);
    Ok(())
}

fn build_rail_menu<R: Runtime>(
    app: &AppHandle<R>,
    server: &Server,
    workspace_id: Option<&str>,
) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    if let Some(id) = workspace_id {
        menu.append(&MenuItem::with_id(
            app,
            format!("hide:{id}:{}", server.url),
            "Hide from Sidebar",
            true,
            None::<&str>,
        )?)?;
    }
    if !server.hidden.is_empty() {
        let shown = match server.hidden.len() {
            1 => "Show 1 Hidden Workspace".to_owned(),
            n => format!("Show {n} Hidden Workspaces"),
        };
        menu.append(&MenuItem::with_id(
            app,
            format!("show-hidden:{}", server.url),
            shown,
            true,
            None::<&str>,
        )?)?;
    }
    if !menu.items()?.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
    }
    menu.append(&MenuItem::with_id(
        app,
        format!("remove:{}", server.url),
        format!("Remove {}…", server.name),
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

#[tauri::command]
fn rail_menu(
    app: AppHandle,
    window: Window,
    url: String,
    workspace_id: Option<String>,
) -> Result<(), String> {
    let server = find(&app, &url)?;
    let workspace = workspace_id.filter(|id| workspaces::is_id(id));
    let menu = build_rail_menu(&app, &server, workspace.as_deref()).map_err(|e| e.to_string())?;
    window.popup_menu(&menu).map_err(|e| e.to_string())
}

fn dispatch<R: Runtime>(app: &AppHandle<R>, url: &str, command: serde_json::Value) {
    if let Some(webview) = app.get_webview(&webview_label(url)) {
        let _ = webview.eval(format!(
            "window.chatSystemsDesktop && window.chatSystemsDesktop.dispatch({command})"
        ));
    }
}

fn server_of<R: Runtime>(app: &AppHandle<R>, webview: &Webview<R>) -> Result<Server, String> {
    app.state::<Servers>()
        .lock()
        .map_err(|e| e.to_string())?
        .servers()
        .iter()
        .find(|s| webview_label(&s.url) == webview.label())
        .cloned()
        .ok_or_else(|| "Only a server's own page can ask this".to_owned())
}

#[tauri::command]
fn report_workspaces(
    app: AppHandle,
    webview: Webview,
    workspaces: Vec<workspaces::Reported>,
    current: Option<String>,
) -> Result<(), String> {
    let server = server_of(&app, &webview)?;
    let origin = Url::parse(&server.url).map_err(|e| e.to_string())?;
    let report = Report {
        workspaces: workspaces::accept(&origin, workspaces),
        current: current.filter(|id| workspaces::is_id(id)),
    };
    app.state::<Reports>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .insert(server.url, report);
    announce(&app);
    Ok(())
}

fn show_notification<R: Runtime>(app: &AppHandle<R>, server: &Server, note: &notifications::Note) {
    #[cfg(target_os = "macos")]
    if !notify_rust::request_auth_blocking().unwrap_or(false) {
        return;
    }
    let Ok(handle) = Notification::new()
        .summary(&note.title)
        .subtitle(if note.subtitle.is_empty() {
            &server.name
        } else {
            &note.subtitle
        })
        .body(&note.body)
        .timeout(Timeout::Milliseconds(NOTIFICATION_LINGERS_MS))
        .show()
    else {
        return;
    };
    let _ = handle.wait_for_response(|response: &NotificationResponse| {
        if matches!(response, NotificationResponse::Default) && open_server(app, server).is_ok() {
            dispatch(
                app,
                &server.url,
                serde_json::json!({ "kind": "open-notification", "key": note.key }),
            );
        }
    });
}

#[tauri::command]
fn notify(
    app: AppHandle,
    webview: Webview,
    notification: notifications::Requested,
) -> Result<(), String> {
    let server = server_of(&app, &webview)?;
    let note = notifications::accept(notification)
        .ok_or_else(|| "A notification needs a key and a title".to_owned())?;
    if note
        .workspace_id
        .as_ref()
        .is_some_and(|id| server.hidden.contains(id))
    {
        return Ok(());
    }
    std::thread::Builder::new()
        .name("notification".into())
        .spawn(move || show_notification(&app, &server, &note))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_workspace(app: AppHandle, url: String, workspace_id: String) -> Result<(), String> {
    if !workspaces::is_id(&workspace_id) {
        return Err(format!("{workspace_id} is not a workspace"));
    }
    open_server(&app, &find(&app, &url)?)?;
    dispatch(
        &app,
        &url,
        serde_json::json!({ "kind": "open-workspace", "workspaceId": workspace_id }),
    );
    Ok(())
}

#[tauri::command]
fn create_workspace(app: AppHandle, url: String) -> Result<(), String> {
    open_server(&app, &find(&app, &url)?)?;
    dispatch(
        &app,
        &url,
        serde_json::json!({ "kind": "create-workspace" }),
    );
    Ok(())
}

#[tauri::command]
fn switch_server(app: AppHandle, url: String) -> Result<(), String> {
    open_server(&app, &find(&app, &url)?)
}

#[tauri::command]
fn show_add_server(app: AppHandle) -> Result<(), String> {
    open_picker(&app)
}

fn bring_back<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let window = main_window(app)?;
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    Ok(())
}

fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let at_login = app.autolaunch().is_enabled().unwrap_or(false);
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "show", "Open Chat Systems", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &CheckMenuItem::with_id(
                app,
                "open-at-login",
                "Open at Login",
                true,
                at_login,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit Chat Systems", true, None::<&str>)?,
        ],
    )
}

fn toggle_open_at_login<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let changed = if autolaunch.is_enabled().unwrap_or(false) {
        autolaunch.disable()
    } else {
        autolaunch.enable()
    };
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY), build_tray_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
    changed.map_err(|e| e.to_string())
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let result = match id.split_once(':') {
        None if id == "add-server" => open_picker(app),
        None if id == "show" => bring_back(app),
        None if id == "open-at-login" => toggle_open_at_login(app),
        None if id == "quit" => {
            app.exit(0);
            Ok(())
        }
        Some(("server", url)) => find(app, url).and_then(|server| open_server(app, &server)),
        Some(("remove", url)) => confirm_remove(app, url),
        Some(("show-hidden", url)) => change_hidden(app, |store| store.show_hidden(url)),
        Some(("hide", rest)) => match rest.split_once(':') {
            Some((workspace, url)) => change_hidden(app, |store| store.hide(url, workspace)),
            None => Ok(()),
        },
        _ => Ok(()),
    };
    if let Err(e) = result {
        eprintln!("{id}: {e}");
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![STARTED_HIDDEN]),
        ))
        .invoke_handler(tauri::generate_handler![
            list_servers,
            add_server,
            remove_server,
            switch_server,
            show_add_server,
            report_workspaces,
            notify,
            rail_menu,
            open_workspace,
            create_workspace
        ])
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage::<Servers>(Mutex::new(Store::load(&dir)));
            app.manage(Showing::default());
            app.manage(Reports::default());
            #[cfg(target_os = "macos")]
            std::thread::spawn(|| {
                let _ = notify_rust::request_auth_blocking();
            });

            let window = WindowBuilder::new(app, MAIN)
                .title("Chat Systems")
                .inner_size(1280.0, 860.0)
                .min_inner_size(560.0, 560.0)
                .build()?;
            let size = window
                .inner_size()?
                .to_logical::<f64>(window.scale_factor()?);
            window.add_child(
                WebviewBuilder::new(RAIL, WebviewUrl::App("rail.html".into())),
                LogicalPosition::new(0.0, 0.0),
                LogicalSize::new(RAIL_WIDTH, size.height),
            )?;
            let watched = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    layout(&watched)
                }
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = watched.hide();
                }
                _ => {}
            });

            let mut tray = TrayIconBuilder::with_id(TRAY)
                .tooltip("Chat Systems")
                .menu(&build_tray_menu(app.handle())?)
                .show_menu_on_left_click(true);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            app.set_menu(build_menu(app.handle())?)?;
            app.on_menu_event(|app, event| on_menu(app, event.id().as_ref()));
            load_all(app.handle())?;
            open_last(app.handle())?;
            if std::env::args().any(|arg| arg == STARTED_HIDDEN) {
                window.hide()?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("the desktop app failed to start");

    app.run(|app, event| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen {
            has_visible_windows: false,
            ..
        } = event
        {
            let _ = bring_back(app);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
