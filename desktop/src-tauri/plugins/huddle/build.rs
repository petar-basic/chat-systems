const COMMANDS: &[&str] = &["begin", "end"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
