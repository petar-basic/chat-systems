#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod phone;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(desktop)]
    desktop::run();
    #[cfg(mobile)]
    phone::run();
}
