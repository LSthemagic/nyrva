fn main() {
    let observatory = tauri_build::InlinedPlugin::new().commands(&[
        "read_cockpit", "read_history", "save_settings", "export_settings",
        "import_settings", "rollback_settings", "clear_history",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().plugin("observatory", observatory))
        .expect("failed to build the desktop capability manifest");

    // tauri-build embeds resources for the application binary, not Cargo examples.
    // The native acceptance example also imports Common Controls v6 functions.
    println!("cargo:rerun-if-changed=examples/experience-smoke.manifest.xml");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = std::path::PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("Cargo package directory is required"),
        ).join("examples/experience-smoke.manifest.xml");
        println!("cargo:rustc-link-arg-examples=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-examples=/MANIFESTINPUT:{}", manifest.display());
    }
}
