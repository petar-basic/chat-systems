use tauri::{Runtime, Window};

#[cfg(target_os = "macos")]
pub fn place_lights<R: Runtime>(window: &Window<R>, across: f64) {
    use objc2_app_kit::{NSWindow, NSWindowButton};
    use objc2_core_foundation::CGPoint;

    const HEIGHT: f64 = 40.0;

    let Ok(pointer) = window.ns_window() else {
        return;
    };
    let ns_window: &NSWindow = unsafe { &*pointer.cast::<NSWindow>() };
    let buttons = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .map(|kind| ns_window.standardWindowButton(kind));
    let [Some(close), Some(minimize), Some(zoom)] = buttons else {
        return;
    };
    let Some(container) =
        (unsafe { close.superview() }).and_then(|view| unsafe { view.superview() })
    else {
        return;
    };

    let mut bar = container.frame();
    bar.size.height = HEIGHT;
    bar.origin.y = ns_window.frame().size.height - HEIGHT;
    container.setFrame(bar);

    let size = close.frame().size;
    let gap = minimize.frame().origin.x - close.frame().origin.x;
    let left = ((across - (2.0 * gap + size.width)) / 2.0).max(0.0);
    let top = (HEIGHT - size.height) / 2.0;
    for (i, button) in [close, minimize, zoom].iter().enumerate() {
        button.setFrameOrigin(CGPoint::new(left + i as f64 * gap, top));
    }
}

#[cfg(not(target_os = "macos"))]
pub fn place_lights<R: Runtime>(_window: &Window<R>, _across: f64) {}
