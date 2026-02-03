fn main() {
    // Link ApplicationServices framework for AXIsProcessTrusted on macOS
    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
    }
    
    tauri_build::build()
}
