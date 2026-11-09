use tauri::{Runtime, Window};

#[cfg(target_os = "macos")]
pub fn keep_lights<R: Runtime>(window: &Window<R>, across: f64) {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2_app_kit::{NSWindow, NSWindowDidUpdateNotification};
    use objc2_foundation::NSNotificationCenter;

    let Ok(pointer) = window.ns_window() else {
        return;
    };
    let Some(ns_window) = (unsafe { Retained::retain(pointer.cast::<NSWindow>()) }) else {
        return;
    };
    place_lights(&ns_window, across);
    let watched = ns_window.clone();
    let block = RcBlock::new(move |_| place_lights(&watched, across));
    let observer = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(NSWindowDidUpdateNotification),
            Some(&ns_window),
            None,
            &block,
        )
    };
    std::mem::forget(observer);
}

#[cfg(target_os = "macos")]
fn place_lights(window: &objc2_app_kit::NSWindow, across: f64) {
    use objc2_app_kit::NSWindowButton;
    use objc2_core_foundation::CGPoint;

    const BAR: f64 = 40.0;

    let buttons = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ]
    .map(|kind| window.standardWindowButton(kind));
    let [Some(close), Some(minimize), Some(zoom)] = buttons else {
        return;
    };
    let Some(holder) = (unsafe { close.superview() }) else {
        return;
    };

    let size = close.frame().size;
    let gap = minimize.frame().origin.x - close.frame().origin.x;
    let left = ((across - (2.0 * gap + size.width)) / 2.0).max(0.0);
    let top = (BAR - size.height) / 2.0;
    let y = if holder.isFlipped() {
        top
    } else {
        holder.frame().size.height - top - size.height
    };
    for (i, button) in [close, minimize, zoom].iter().enumerate() {
        let target = CGPoint::new(left + i as f64 * gap, y);
        let now = button.frame().origin;
        if (now.x - target.x).abs() > 0.5 || (now.y - target.y).abs() > 0.5 {
            button.setFrameOrigin(target);
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn keep_lights<R: Runtime>(_window: &Window<R>, _across: f64) {}
