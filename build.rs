// SPDX-FileCopyrightText: 2026 Raimo Geisel
// SPDX-License-Identifier: GPL-2.0

//! Build script for podfetch.
//!
//! When cross-compiling for Windows, embeds icon as the application icon
//! so Explorer and the taskbar display it correctly.
//! 
//! Requires icon.ico to exist. The release script generates it from icon.png.

use std::path::PathBuf;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let out_dir = std::env::var("OUT_DIR").unwrap();
        
        let manifest_path = PathBuf::from(&manifest_dir);
        let out_path = PathBuf::from(&out_dir);
        
        let ico_src = manifest_path.join("icon.ico");

        if !ico_src.exists() {
            println!(
                "cargo:warning=icon.ico not found. Windows exe will have no icon."
            );
            println!(
                "cargo:warning=To generate icon.ico, run: convert icon.png -define icon:auto-resize=256,128,64,32,16 icon.ico"
            );
            println!(
                "cargo:warning=(requires imagemagick, or use the release script which handles this)"
            );
            return;
        }

        let ico_dest = out_path.join("podfetch.ico");
        std::fs::copy(&ico_src, &ico_dest).expect("failed to copy icon to OUT_DIR");

        let rc_path = out_path.join("podfetch.rc");
        std::fs::write(&rc_path, "1 ICON \"podfetch.ico\"\n").expect("failed to write resource script");

        let obj_path = out_path.join("podfetch_icon.o");
        let windres = "x86_64-w64-mingw32-windres";
        let status = std::process::Command::new(windres)
            .args([
                rc_path.to_str().unwrap(),
                "-o",
                obj_path.to_str().unwrap(),
                "--target=pe-x86-64",
            ])
            .current_dir(&out_path)
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
                    "cargo:warning=Could not run {windres}: {e}; Windows exe will have no icon",
                );
            }
        }
    }
}
