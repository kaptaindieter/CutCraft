#![allow(missing_docs)]
// SPDX-License-Identifier: AGPL-3.0-or-later
// ArtCraft Suite — Google Drive Cloud Synchronization for CutCraft

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct ArtCraftDrive {
    pub app_name: &'static str,
    pub target_dir: PathBuf,
    pub is_google_drive: bool,
}

impl ArtCraftDrive {
    pub fn user_configured_sync_root() -> Option<(String, PathBuf)> {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        let cfg_path = PathBuf::from(home).join(".craft-suite").join("cloud-config.json");
        let raw = fs::read_to_string(cfg_path).ok()?;
        let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
        let sync_path = val.get("sync_path")?.as_str()?.trim();
        let provider = val
            .get("provider")
            .and_then(|p| p.as_str())
            .unwrap_or("Cloud Sync")
            .trim()
            .to_owned();
        if sync_path.is_empty() {
            None
        } else {
            let p = PathBuf::from(sync_path);
            let target = if p.ends_with("CutCraft") {
                p
            } else {
                p.join("Art Craft").join("CutCraft")
            };
            Some((provider, target))
        }
    }

    pub fn new() -> Self {
        if let Some((_provider, target)) = Self::user_configured_sync_root() {
            let _ = fs::create_dir_all(&target);
            return Self {
                app_name: "CutCraft",
                target_dir: target,
                is_google_drive: true,
            };
        }

        let drive_candidates = [
            Path::new(r"G:\My Drive\Art Craft\CutCraft"),
            Path::new(r"G:\Meine Ablage\Art Craft\CutCraft"),
            Path::new(r"G:\Art Craft\CutCraft"),
        ];

        for c in drive_candidates {
            if c.exists() {
                return Self {
                    app_name: "CutCraft",
                    target_dir: c.to_path_buf(),
                    is_google_drive: true,
                };
            }
        }

        let drive_roots = [
            Path::new(r"G:\My Drive"),
            Path::new(r"G:\Meine Ablage"),
            Path::new(r"G:\"),
        ];

        for r in drive_roots {
            if r.exists() && r.is_dir() {
                let target = r.join("Art Craft").join("CutCraft");
                let _ = fs::create_dir_all(&target);
                if target.exists() {
                    return Self {
                        app_name: "CutCraft",
                        target_dir: target,
                        is_google_drive: true,
                    };
                }
            }
        }

        // Safe fallback: User Documents (Never Desktop!)
        let fallback = dirs::document_dir()
            .unwrap_or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|p| PathBuf::from(p).join("Documents"))
                    .unwrap_or_else(|| PathBuf::from(r"C:\ArtCraft_Drive"))
            })
            .join("Art Craft")
            .join("CutCraft");
        let _ = fs::create_dir_all(&fallback);
        Self {
            app_name: "CutCraft",
            target_dir: fallback,
            is_google_drive: false,
        }
    }

    pub fn status_label(&self) -> String {
        if let Some((provider, _)) = Self::user_configured_sync_root() {
            format!("{provider}: Active")
        } else if self.is_google_drive {
            "Google Drive: Active".to_owned()
        } else {
            "Local Sync: Active".to_owned()
        }
    }

    pub fn save_custom_sync_root(&mut self, chosen_folder: PathBuf) {
        if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
            let cfg_dir = PathBuf::from(home).join(".craft-suite");
            let _ = fs::create_dir_all(&cfg_dir);
            let cfg_file = cfg_dir.join("cloud-config.json");
            let json = serde_json::json!({
                "provider": "Custom Sync",
                "sync_path": chosen_folder.to_string_lossy().to_string(),
                "auto_save": true
            });
            if let Ok(s) = serde_json::to_string_pretty(&json) {
                let _ = fs::write(cfg_file, s);
            }
        }
        let target = if chosen_folder.ends_with("CutCraft") {
            chosen_folder
        } else {
            chosen_folder.join("Art Craft").join("CutCraft")
        };
        let _ = fs::create_dir_all(&target);
        self.target_dir = target;
        self.is_google_drive = true;
    }

    pub fn open_folder(&self, active_project: Option<&Path>) {
        let folder = active_project
            .filter(|p| p.exists())
            .unwrap_or(self.target_dir.as_path());
        let _ = fs::create_dir_all(folder);
        #[cfg(target_os = "windows")]
        {
            let _ = Command::new("explorer").arg(folder).spawn();
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = opener::open(folder);
        }
    }

    /// Mirrors a saved project's `project.concat` and `concat.json` files into the Google Drive directory
    /// if the project is located outside of Google Drive.
    pub fn sync_project_dir(&self, project_dir: &Path) {
        if !project_dir.exists() {
            return;
        }
        if project_dir.starts_with(&self.target_dir) {
            return;
        }
        let Some(folder_name) = project_dir.file_name() else {
            return;
        };
        let dest_dir = self.target_dir.join(folder_name);
        let _ = fs::create_dir_all(&dest_dir);

        for filename in ["project.concat", "concat.json"] {
            let src = project_dir.join(filename);
            if src.exists() {
                let _ = fs::copy(&src, dest_dir.join(filename));
            }
        }
    }
}

pub fn default_projects_dir() -> String {
    let drive = ArtCraftDrive::new();
    drive.target_dir.to_string_lossy().into_owned()
}
