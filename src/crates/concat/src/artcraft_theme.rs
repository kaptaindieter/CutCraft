#![allow(missing_docs)]
// SPDX-License-Identifier: AGPL-3.0-or-later
// ArtCraft Suite — Global Color Theme Persistence (✨ Aurora, 🌙 Dark, ☀️ Light)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Theme mode shared across all ArtCraft applications:
/// 0 = ✨ Aurora, 1 = 🌙 Dark, 2 = ☀️ Light
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtCraftThemeMode {
    Aurora,
    Dark,
    Light,
}

impl ArtCraftThemeMode {
    pub fn to_index(self) -> i32 {
        match self {
            Self::Aurora => 0,
            Self::Dark => 1,
            Self::Light => 2,
        }
    }

    pub fn from_index(idx: i32) -> Self {
        match idx {
            1 => Self::Dark,
            2 => Self::Light,
            _ => Self::Aurora,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Aurora => "aurora",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SharedThemeConfig {
    pub theme: String,
}

fn shared_theme_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(prof) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let base = PathBuf::from(prof);
        let craft_dir = base.join(".craft-suite");
        let _ = fs::create_dir_all(&craft_dir);
        paths.push(craft_dir.join("artcraft_theme.json"));

        let dir = base.join(".artcraft");
        let _ = fs::create_dir_all(&dir);
        paths.push(dir.join("artcraft_theme.json"));
    }

    if let Some(appdata) = std::env::var_os("APPDATA") {
        let dir = PathBuf::from(appdata).join("ArtCraft");
        let _ = fs::create_dir_all(&dir);
        paths.push(dir.join("artcraft_theme.json"));
    }
    paths
}

pub fn load_theme_mode() -> ArtCraftThemeMode {
    for path in shared_theme_paths() {
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<SharedThemeConfig>(&raw) {
                return match cfg.theme.trim().to_ascii_lowercase().as_str() {
                    "dark" => ArtCraftThemeMode::Dark,
                    "light" => ArtCraftThemeMode::Light,
                    _ => ArtCraftThemeMode::Aurora,
                };
            }
        }
    }
    ArtCraftThemeMode::Aurora
}

pub fn save_theme_mode(mode: ArtCraftThemeMode) {
    let cfg = SharedThemeConfig {
        theme: mode.as_str().to_owned(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&cfg) {
        for path in shared_theme_paths() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&path, &json);
        }
    }
}
