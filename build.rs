// SPDX-FileCopyrightText: 2026 Raimo Geisel
// SPDX-License-Identifier: GPL-2.0

//! Build script for podfetch.
//!
//! When cross-compiling for Windows, embeds icon.ico as the application icon
//! so Explorer and the taskbar display it correctly.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let ico_src = std::path::PathBuf::from(&manifest_dir).join("icon.ico");

        if !ico_src.exists() {
            println!(
                "cargo:warning=icon.ico not found; Windows exe will have no icon. Run the release script or generate it from icon.png."
            );
            return;
        }

        let ico_dest = std::path::PathBuf::from(&out_dir).join("podfetch.ico");
        std::fs::copy(&ico_src, &ico_dest).expect("failed to copy icon to OUT_DIR");

        let rc_path = std::path::PathBuf::from(&out_dir).join("podfetch.rc");
        std::fs::write(&rc_path, "1 ICON \"podfetch.ico\"
").expect("failed to write resource script");

        let obj_path = std::path::PathBuf::from(&out_dir).join("podfetch_icon.o");
        let windres = "x86_64-w64-mingw32-windres";
        let status = std::process::Command::new(windres)
            .args([
                rc_path.to_str().unwrap(),
                "-o",
                obj_path.to_str().unwrap(),
                "--target=pe-x86-64",
            ])
            .current_dir(&out_dir)
            .status();

        match status {
            Ok(s) if s.success() => {
                println!("cargo:rustc-link-arg={}", obj_path.display());
            }
            Ok(s) => {
                println!("cargo:warning=windres exited with {s}; Windows exe will have no icon");
            }
            Err(e) => {
                println!(
                    "cargo:warning=Could not run {windres}: {e}; Windows exe will have no icon"
                );
            }
        }
    }
}
