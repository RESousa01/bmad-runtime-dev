fn main() {
    #[cfg(target_os = "windows")]
    {
        // Windows installer detection treats a test executable containing
        // "update" as an installer unless it declares an execution level.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:level='asInvoker' uiAccess='false'");
    }
}
