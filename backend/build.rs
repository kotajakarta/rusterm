fn main() {
    // Only embed Windows resources when compiling for Windows target
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        let icon_path = std::path::Path::new(&manifest_dir).join("app_icon.ico");
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());

        let mut res = winres::WindowsResource::new();
        if icon_path.exists() {
            res.set_icon(icon_path.to_str().unwrap());
        } else {
            res.set_icon("app_icon.ico");
        }

        res.set("ProductName", "RusTerm");
        res.set("FileDescription", "RusTerm Terminal Deck");
        res.set("OriginalFilename", "rusterm.exe");

        let target = std::env::var("TARGET").unwrap_or_default();
        if target.contains("windows-gnu") {
            if std::path::Path::new("/usr/bin/x86_64-w64-mingw32-windres").exists() {
                res.set_windres_path("x86_64-w64-mingw32-windres");
            }
            if std::path::Path::new("/usr/bin/x86_64-w64-mingw32-ar").exists() {
                res.set_ar_path("x86_64-w64-mingw32-ar");
            }
        }

        if let Err(e) = res.compile() {
            eprintln!("Windows resource compilation error: {}", e);
            std::process::exit(1);
        }

        // MinGW ld drops static archives if no code symbols are referenced.
        // Linking resource.o directly ensures .rsrc section is included in the final PE binary.
        if target.contains("windows-gnu") {
            let res_o = out_dir.join("resource.o");
            if res_o.exists() {
                println!("cargo:rustc-link-arg={}", res_o.display());
            }
        }
    }
}
