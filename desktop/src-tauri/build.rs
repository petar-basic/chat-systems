fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "list_servers",
            "add_server",
            "remove_server",
            "switch_server",
            "show_add_server",
            "report_workspaces",
            "open_workspace",
            "create_workspace",
        ]),
    ))
    .expect("tauri build script");
}
