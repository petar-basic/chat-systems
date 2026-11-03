use std::time::Duration;

use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;

const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

pub fn is_configured<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.config().plugins.0.contains_key("updater")
}

pub fn keep_updated<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            if let Err(e) = update(&app).await {
                eprintln!("update: {e}");
            }
            tokio::time::sleep(CHECK_EVERY).await;
        }
    });
}

async fn update<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let Some(update) = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(());
    };
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    let answer = rfd::AsyncMessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title(format!("Chat Systems {} is ready", update.version))
        .set_description("Restart now to finish updating, or keep working and update later.")
        .set_buttons(rfd::MessageButtons::OkCancelCustom(
            "Restart Now".into(),
            "Later".into(),
        ))
        .show()
        .await;
    let restart = match answer {
        rfd::MessageDialogResult::Ok => true,
        rfd::MessageDialogResult::Custom(label) => label == "Restart Now",
        _ => false,
    };
    if restart {
        update.install(bytes).map_err(|e| e.to_string())?;
        app.restart();
    }
    Ok(())
}
