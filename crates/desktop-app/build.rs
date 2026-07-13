fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "host_bootstrap",
        "host_dispatch",
        "host_projection_snapshot",
        "host_projection_events",
    ]);
    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("windows-app-manifest.xml"));
    let attributes = tauri_build::Attributes::new()
        .app_manifest(manifest)
        .windows_attributes(windows);

    tauri_build::try_build(attributes)?;
    Ok(())
}
