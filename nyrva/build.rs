fn main() {
    let observatory = tauri_build::InlinedPlugin::new().commands(&[
        "read_cockpit", "read_history", "save_settings", "export_settings",
        "import_settings", "rollback_settings", "clear_history",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().plugin("observatory", observatory))
        .expect("failed to build the desktop capability manifest");
}
