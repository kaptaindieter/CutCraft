// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Compiles the `.slint` tree into the binary and records three facts about
//! the build for Settings > About.

fn main() {
    let compile = std::thread::Builder::new()
        .name("slint".into())
        .stack_size(512 << 20)
        .spawn(|| {
            let config = slint_build::CompilerConfiguration::new()
                .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
            slint_build::compile_with_config("ui/app.slint", config)
        })
        .expect("could not start the Slint compiler thread");
    compile
        .join()
        .expect("the Slint compiler thread panicked")
        .expect("failed to compile ui/app.slint");

    {
        let root =
            std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
                .join("text-presets");
        println!("cargo:rerun-if-changed={}", root.display());
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&root)
            .expect("text-presets/ exists")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path().join("preset.toml"))
            .filter(|file| file.is_file())
            .collect();
        files.sort();
        let mut table = String::from("pub static BUILTIN: &[&str] = &[\n");
        for file in &files {
            println!("cargo:rerun-if-changed={}", file.display());
            table.push_str(&format!(
                "    include_str!({:?}),\n",
                file.display().to_string()
            ));
        }
        table.push_str("];\n");
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("out dir"))
            .join("text_presets.rs");
        std::fs::write(out, table).expect("write text_presets.rs");
    }

    {
        let root =
            std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"))
                .join("cards");
        println!("cargo:rerun-if-changed={}", root.display());
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&root)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| path.extension().is_some_and(|ext| ext == "jpg"))
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        let mut table = String::from("pub static CARDS: &[(&str, &[u8])] = &[\n");
        for file in &files {
            println!("cargo:rerun-if-changed={}", file.display());
            let id = file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .expect("a card's name");
            table.push_str(&format!(
                "    ({id:?}, include_bytes!({:?})),\n",
                file.display().to_string()
            ));
        }
        table.push_str("];\n");
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("out dir"))
            .join("bundled_cards.rs");
        std::fs::write(out, table).expect("write bundled_cards.rs");
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon = "../../../assets/icons/concat.ico";
        println!("cargo:rerun-if-changed={icon}");
        winresource::WindowsResource::new()
            .set_icon(icon)
            .set("ProductName", "CutCraft")
            .set("FileDescription", "CutCraft")
            .set(
                "LegalCopyright",
                "Copyright 2026 ArtCraft Suite & CutCraft contributors, AGPL-3.0-or-later",
            )
            .compile()
            .expect("could not embed the Windows icon");
    }

    println!("cargo:rerun-if-env-changed=RUSTC");
    println!(
        "cargo:rustc-env=BUILD_TARGET={}",
        std::env::var("TARGET").unwrap_or_else(|_| "unknown".into())
    );
    println!(
        "cargo:rustc-env=BUILD_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".into())
    );
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = std::process::Command::new(rustc)
        .arg("-V")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=BUILD_RUSTC={version}");
}
