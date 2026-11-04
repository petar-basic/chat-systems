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
            "notify",
            "rail_menu",
            "add_menu",
            "open_workspace",
            "start_sso",
        ]),
    ))
    .expect("tauri build script");
}
