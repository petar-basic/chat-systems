use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Monitor, Runtime, WebviewUrl, WebviewWindowBuilder};

pub const OVERLAY: &str = "draw-overlay";
const STROKE_EVENT: &str = "overlay-stroke";
const MAX_POINTS: usize = 4096;
const MAX_TEXT: usize = 120;

#[derive(Default)]
pub struct Owner(Mutex<Option<String>>);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Stroke {
    key: String,
    color: String,
    name: String,
    points: Vec<[f64; 2]>,
    fade_at: f64,
    fade_ms: f64,
}

impl Stroke {
    fn is_sane(&self) -> bool {
        self.key.len() <= MAX_TEXT
            && self.name.chars().count() <= MAX_TEXT
            && is_colour(&self.color)
            && self.points.len() <= MAX_POINTS
            && self
                .points
                .iter()
                .all(|p| p.iter().all(|n| (0.0..=1.0).contains(n)))
            && self.fade_at.is_finite()
            && (0.0..=10_000.0).contains(&self.fade_ms)
    }
}

fn is_colour(colour: &str) -> bool {
    colour.len() == 7
        && colour.starts_with('#')
        && colour[1..].chars().all(|c| c.is_ascii_hexdigit())
}

fn pick_monitor(monitors: Vec<Monitor>, width: u32, height: u32) -> Option<Monitor> {
    monitors
        .into_iter()
        .find(|m| m.size().width == width && m.size().height == height)
}

pub fn open<R: Runtime>(
    app: &AppHandle<R>,
    owner: &str,
    width: u32,
    height: u32,
) -> Result<(), String> {
    close(app);
    let main = app.get_window(crate::MAIN);
    let monitor = pick_monitor(
        app.available_monitors().map_err(|e| e.to_string())?,
        width,
        height,
    )
    .or_else(|| main.and_then(|w| w.current_monitor().ok().flatten()))
    .or_else(|| app.primary_monitor().ok().flatten())
    .ok_or("No screen to draw on")?;
    let window = WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("overlay.html".into()))
        .title("Drawing")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .resizable(false)
        .always_on_top(true)
        .visible_on_all_workspaces(true)
        .skip_taskbar(true)
        .content_protected(true)
        .focused(false)
        .focusable(false)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    window
        .set_position(*monitor.position())
        .map_err(|e| e.to_string())?;
    window
        .set_size(*monitor.size())
        .map_err(|e| e.to_string())?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    *app.state::<Owner>().0.lock().map_err(|e| e.to_string())? = Some(owner.to_owned());
    Ok(())
}

fn owned_by<R: Runtime>(app: &AppHandle<R>, owner: &str) -> bool {
    app.state::<Owner>()
        .0
        .lock()
        .is_ok_and(|current| current.as_deref() == Some(owner))
}

pub fn draw<R: Runtime>(app: &AppHandle<R>, owner: &str, stroke: Stroke) -> Result<(), String> {
    if !owned_by(app, owner) {
        return Err("Only the page sharing the screen draws on it".into());
    }
    if !stroke.is_sane() {
        return Err("Not a stroke".into());
    }
    app.emit_to(OVERLAY, STROKE_EVENT, stroke)
        .map_err(|e| e.to_string())
}

pub fn close_for<R: Runtime>(app: &AppHandle<R>, owner: &str) {
    if owned_by(app, owner) {
        close(app);
    }
}

fn close<R: Runtime>(app: &AppHandle<R>) {
    if let Ok(mut current) = app.state::<Owner>().0.lock() {
        *current = None;
    }
    if let Some(window) = app.get_webview_window(OVERLAY) {
        let _ = window.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke() -> Stroke {
        Stroke {
            key: "u-1:7".into(),
            color: "#a855f7".into(),
            name: "Bob".into(),
            points: vec![[0.0, 0.5], [1.0, 0.25]],
            fade_at: 1_700_000_000_000.0,
            fade_ms: 500.0,
        }
    }

    #[test]
    fn a_stroke_from_the_huddle_is_drawn() {
        assert!(stroke().is_sane());
    }

    #[test]
    fn anything_else_is_not() {
        let off_screen = Stroke {
            points: vec![[1.2, 0.5]],
            ..stroke()
        };
        let styled = Stroke {
            color: "red; background: url(x)".into(),
            ..stroke()
        };
        let endless = Stroke {
            points: vec![[0.5, 0.5]; MAX_POINTS + 1],
            ..stroke()
        };
        let lingering = Stroke {
            fade_ms: f64::INFINITY,
            ..stroke()
        };
        for bad in [off_screen, styled, endless, lingering] {
            assert!(!bad.is_sane(), "{bad:?}");
        }
    }
}
