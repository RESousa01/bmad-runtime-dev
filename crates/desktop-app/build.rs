use std::{env, fs, io, path::Path};

fn make_files_writable(root: &Path) -> io::Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            make_files_writable(&path)?;
        } else {
            let mut permissions = fs::metadata(&path)?.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                fs::set_permissions(path, permissions)?;
            }
        }
    }
    Ok(())
}

fn prepare_resource_cache() -> io::Result<()> {
    let out_dir = env::var_os("OUT_DIR")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "OUT_DIR is missing"))?;
    let target_profile = out_dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "OUT_DIR has no target profile"))?;
    make_files_writable(&target_profile.join("bmad-foundation"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    prepare_resource_cache()?;
    let manifest = tauri_build::AppManifest::new().commands(&[
        "host_bootstrap",
        "host_dispatch",
        "host_projection_snapshot",
        "host_projection_events",
        "install_app_update",
    ]);
    let windows = tauri_build::WindowsAttributes::new()
        .app_manifest(include_str!("windows-app-manifest.xml"));
    let attributes = tauri_build::Attributes::new()
        .app_manifest(manifest)
        .windows_attributes(windows);

    tauri_build::try_build(attributes)?;
    println!("cargo:rerun-if-env-changed=SAPPHIRUS_UPDATE_ENDPOINT");
    println!("cargo:rerun-if-env-changed=SAPPHIRUS_UPDATE_PUBLIC_KEY");
    Ok(())
}
