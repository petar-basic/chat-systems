fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "list_servers",
            "add_server",
            "remove_server",
            "switch_server",
            "report_workspaces",
            "report_theme",
            "report_colours",
            "report_creation",
            "notify",
            "rail_menu",
            "rail_hint",
            "add_menu",
            "open_workspace",
            "start_sso",
            "start_passkey",
            "press_menu_keys",
        ]),
    ))
    .expect("tauri build script");
}
