use tauri::webview::{NewWindowFeatures, NewWindowResponse};
use tauri::{AppHandle, Runtime, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use url::Url;

#[cfg(target_os = "ios")]
use objc2_ui_kit::{UIColor, UIView};

const MAIN: &str = "main";

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle();
            #[cfg_attr(target_os = "android", allow(unused_variables))]
            let window = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
                .on_navigation(stay_in_app(handle))
                .on_new_window(open_outside(handle))
                .build()?;
            #[cfg(target_os = "ios")]
            window.with_webview(|webview| keep_clear_of_status_bar(webview.inner()))?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the app could not start");
}

#[cfg(target_os = "ios")]
fn keep_clear_of_status_bar(webview: *mut std::ffi::c_void) {
    let view = unsafe { &*webview.cast::<UIView>() };
    let Some(parent) = view.superview() else {
        return;
    };
    parent.setBackgroundColor(Some(&UIColor::systemBackgroundColor()));
    let safe = parent.safeAreaLayoutGuide();
    view.setTranslatesAutoresizingMaskIntoConstraints(false);
    for constraint in [
        view.topAnchor().constraintEqualToAnchor(&safe.topAnchor()),
        view.bottomAnchor()
            .constraintEqualToAnchor(&parent.bottomAnchor()),
        view.leadingAnchor()
            .constraintEqualToAnchor(&safe.leadingAnchor()),
        view.trailingAnchor()
            .constraintEqualToAnchor(&safe.trailingAnchor()),
    ] {
        constraint.setActive(true);
    }
}

fn in_app(target: &Url) -> bool {
    matches!(target.scheme(), "tauri" | "about" | "blob" | "data")
        || target.host_str() == Some("tauri.localhost")
}

fn open_externally<R: Runtime>(app: &AppHandle<R>, target: &Url) {
    if matches!(target.scheme(), "https" | "http" | "mailto" | "tel") {
        let _ = app.opener().open_url(target.as_str(), None::<&str>);
    }
}

fn stay_in_app<R: Runtime>(app: &AppHandle<R>) -> impl Fn(&Url) -> bool + Send + 'static {
    let app = app.clone();
    move |target| {
        if in_app(target) {
            return true;
        }
        open_externally(&app, target);
        false
    }
}

fn open_outside<R: Runtime>(
    app: &AppHandle<R>,
) -> impl Fn(Url, NewWindowFeatures) -> NewWindowResponse<R> + Send + 'static {
    let app = app.clone();
    move |target, _| {
        open_externally(&app, &target);
        NewWindowResponse::Deny
    }
}
