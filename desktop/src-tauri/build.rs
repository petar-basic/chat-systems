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
            "open_link",
            "notify",
            "rail_menu",
            "rail_hint",
            "add_menu",
            "open_workspace",
            "start_sso",
            "start_passkey",
            "press_menu_keys",
            "open_draw_overlay",
            "draw_on_overlay",
            "close_draw_overlay",
        ]),
    ))
    .expect("tauri build script");
}
