fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "host_bootstrap",
        "host_dispatch",
        "host_projection_snapshot",
        "host_projection_events",
    ]);

    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(manifest))
        .unwrap_or_else(|error| panic!("failed to build the Sapphirus desktop manifest: {error}"));
}
