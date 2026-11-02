mod navigation;
mod servers;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::webview::{DownloadEvent, NewWindowFeatures, NewWindowResponse, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, RunEvent, Runtime, State, Webview,
    WebviewUrl, WebviewWindowBuilder, Window, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

use servers::{Server, Store};

const MAIN: &str = "main";
const RAIL: &str = "rail";
const PICKER: &str = "picker";
const RAIL_WIDTH: f64 = 72.0;

static VIEWERS: AtomicUsize = AtomicUsize::new(0);

type Servers = Mutex<Store>;

#[derive(Default)]
struct Showing(Mutex<String>);

#[derive(Clone, Serialize)]
struct Overview {
    servers: Vec<Server>,
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
    Overview { servers, active }
}

fn announce<R: Runtime>(app: &AppHandle<R>) {
    let now = overview(app);
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

fn open_server<R: Runtime>(app: &AppHandle<R>, server: &Server) -> Result<(), String> {
    let window = main_window(app)?;
    let label = webview_label(&server.url);
    if window.webviews().iter().all(|w| w.label() != label) {
        let origin = Url::parse(&server.url).map_err(|e| e.to_string())?;
        let builder = WebviewBuilder::new(&label, WebviewUrl::External(origin.clone()))
            .on_navigation(stay_on(app, origin.clone()))
            .on_new_window(popups(app, origin))
            .on_download(save_downloads);
        let (position, size) = content_bounds(&window);
        window
            .add_child(builder, position, size)
            .map_err(|e| e.to_string())?;
    }
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

#[tauri::command]
fn remove_server(app: AppHandle, servers: State<'_, Servers>, url: String) -> Result<(), String> {
    servers.lock().map_err(|e| e.to_string())?.remove(&url)?;
    let label = webview_label(&url);
    let was_showing = app
        .state::<Showing>()
        .0
        .lock()
        .map(|showing| *showing == label)
        .unwrap_or(false);
    if let Some(webview) = app.get_webview(&label) {
        let _ = webview.close();
    }
    if was_showing {
        return open_last(&app);
    }
    announce(&app);
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

fn on_menu<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let result = match (id, id.strip_prefix("server:")) {
        ("add-server", _) => open_picker(app),
        (_, Some(url)) => find(app, url).and_then(|server| open_server(app, &server)),
        _ => Ok(()),
    };
    if let Err(e) = result {
        eprintln!("{id}: {e}");
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            list_servers,
            add_server,
            remove_server,
            switch_server,
            show_add_server
        ])
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            app.manage::<Servers>(Mutex::new(Store::load(&dir)));
            app.manage(Showing::default());

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
            let resized = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } = event {
                    layout(&resized);
                }
            });

            app.set_menu(build_menu(app.handle())?)?;
            app.on_menu_event(|app, event| on_menu(app, event.id().as_ref()));
            open_last(app.handle()).map_err(|e| e.into())
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
            if let Ok(window) = main_window(app) {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = (app, event);
    });
}
