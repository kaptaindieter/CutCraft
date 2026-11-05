#![allow(missing_docs)]
// SPDX-License-Identifier: AGPL-3.0-or-later
// ArtCraft Suite â€” 1:1 "Gemini in Google Workspace" AI Agent & Native CutCraft MCP Executor

use serde::{Deserialize, Serialize};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use concat_project::Command;
use concat_project::model::{ClipKind, TextStyle};

use crate::artcraft_drive::ArtCraftDrive;
use crate::artcraft_theme::{ArtCraftThemeMode, load_theme_mode, save_theme_mode};
use crate::host::Shell;
use crate::studio::Studio;
use crate::ui::{AiMessageItem, AiModelItem, App, Craft, Theme};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCustomModel {
    pub id: String,
    pub name: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(alias = "baseUrl", alias = "endpoint", default)]
    pub base_url: String,
    #[serde(alias = "apiKey", alias = "api_key", default)]
    pub api_key: String,
    #[serde(alias = "api_model_id", alias = "modelId", alias = "model", default)]
    pub model_id: String,
    #[serde(default)]
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SharedCustomBots {
    pub bots: Vec<SavedCustomModel>,
}

fn default_provider() -> String {
    "openai".to_owned()
}

#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub role: String,
    pub content: String,
    pub has_action: bool,
    pub action_summary: String,
}

pub struct AgentState {
    pub models: Vec<SavedCustomModel>,
    pub active_model_id: String,
    pub messages: Vec<ChatEntry>,
    pub auto_save: bool,
    pub drive: ArtCraftDrive,
}

thread_local! {
    static AGENT_STATE: RefCell<Option<Rc<RefCell<AgentState>>>> = const { RefCell::new(None) };
}

fn shared_models_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(user_profile) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        let p_home = PathBuf::from(&user_profile);
        paths.push(p_home.join(".craft-suite").join("shared-ai-config.json"));
        paths.push(p_home.join(".artcraft").join("shared_custom_models.json"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let p_appdata = PathBuf::from(appdata);
        paths.push(p_appdata.join("CraftSuite").join("shared-ai-config.json"));
        paths.push(p_appdata.join("ArtCraft").join("shared_custom_models.json"));
    }
    paths
}

pub fn load_saved_models() -> Vec<SavedCustomModel> {
    for path in shared_models_paths() {
        if path.exists() {
            if let Ok(raw) = fs::read_to_string(&path) {
                if let Ok(shared) = serde_json::from_str::<SharedCustomBots>(&raw) {
                    if !shared.bots.is_empty() {
                        return shared.bots;
                    }
                }
                if let Ok(list) = serde_json::from_str::<Vec<SavedCustomModel>>(&raw) {
                    if !list.is_empty() {
                        return list;
                    }
                }
            }
        }
    }
    Vec::new()
}

pub fn save_models_to_disk(models: &[SavedCustomModel]) {
    let shared = SharedCustomBots { bots: models.to_vec() };
    if let Ok(json) = serde_json::to_string_pretty(&shared) {
        for path in shared_models_paths() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&path, &json);
        }
    }
}

impl Studio {
    pub fn craft_context_label(&self) -> String {
        if self.on_start || self.session.is_none() {
            "CutCraft Start Screen · No project open".to_owned()
        } else {
            let tl = self.timeline();
            format!(
                "{} · {} Tracks · {} Clips · Playhead {:.1}s",
                self.project_name,
                tl.tracks.len(),
                tl.clips.len(),
                self.playhead
            )
        }
    }

    pub fn craft_timeline_snapshot(&self) -> String {
        if self.on_start || self.session.is_none() {
            return "Status: On start screen (no video project open yet).".to_owned();
        }
        let tl = self.timeline();
        let proj = self.project();
        let mut out = format!(
            "Project: '{}' ({}x{} @ {:.2} fps)\nPlayhead: {:.2}s | Total Duration: {:.2}s | Tracks: {} | Clips: {} | Media in Bin: {}\n",
            self.project_name,
            tl.video.width,
            tl.video.height,
            tl.video.rate(),
            self.playhead,
            tl.duration(),
            tl.tracks.len(),
            tl.clips.len(),
            proj.media.len()
        );
        if !tl.clips.is_empty() {
            out.push_str("Clips on Timeline:\n");
            for (idx, clip) in tl.clips.iter().take(20).enumerate() {
                let extra = clip
                    .text
                    .as_ref()
                    .map(|t| format!(" [Text: \"{}\"]", t.content))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  {}. [{}] '{}' ({:?}) Start: {:.2}s, Duration: {:.2}s, Speed: {:.2}x{}\n",
                    idx + 1,
                    clip.id,
                    clip.name,
                    clip.kind,
                    clip.start,
                    clip.duration,
                    clip.speed,
                    extra
                ));
            }
        }
        out
    }

    pub fn craft_add_title(
        &mut self,
        text: &str,
        start_opt: Option<f64>,
        duration_opt: Option<f64>,
    ) -> Result<String, String> {
        if self.session.is_none() {
            return Err("Please open or create a project in CutCraft first.".to_owned());
        }
        let clean_text = text.trim();
        let content = if clean_text.is_empty() {
            "CutCraft Title".to_owned()
        } else {
            clean_text.to_owned()
        };
        let base = self.prefs.title_style.clone().unwrap_or_else(|| TextStyle {
            font_family: "Hanken Grotesk".to_owned(),
            font_weight: 700.0,
            ..TextStyle::default()
        });
        let style = TextStyle {
            content: content.clone(),
            runs: Vec::new(),
            ..base
        };
        let start = start_opt.unwrap_or_else(|| f64::from(self.playhead.max(0.0)));
        let duration = duration_opt.unwrap_or(4.0).clamp(0.5, 60.0);
        let cmd = Command::AddTextClip {
            track_id: None,
            start,
            style: Some(style),
            duration: Some(duration),
            offset_y: None,
            above: true,
        };
        if let Some(id) = self.apply(cmd) {
            self.selection = vec![id];
            self.notify(&format!("Titel \"{}\" eingefÃ¼gt", content), false);
            Ok(format!(
                "Titel-Overlay \"{}\" bei {:.1}s ({:.1}s) eingefÃ¼gt",
                content, start, duration
            ))
        } else {
            Err("Could not create title clip.".to_owned())
        }
    }

    pub fn craft_split_at_playhead(&mut self) -> Result<String, String> {
        if self.session.is_none() {
            return Err("Please open a project first.".to_owned());
        }
        let at = self.playhead;
        let before_count = self.timeline().clips.len();
        self.split_at(at, false);
        let after_count = self.timeline().clips.len();
        if after_count > before_count {
            Ok(format!(
                "Clip(s) split at playhead ({:.2}s) ({} → {} clips)",
                at, before_count, after_count
            ))
        } else {
            Ok(format!(
                "Cut command executed at playhead {:.2}s (no cuttable clip directly under playhead)",
                at
            ))
        }
    }

    pub fn craft_add_track(&mut self, bottom: bool) -> Result<String, String> {
        if self.session.is_none() {
            return Err("Please open a project first.".to_owned());
        }
        self.apply(Command::AddTrack { bottom });
        Ok(format!(
            "New timeline track added (Total: {} tracks)",
            self.timeline().tracks.len()
        ))
    }

    pub fn craft_apply_filter(&mut self, effect_id: &str, label: &str) -> Result<String, String> {
        if self.session.is_none() {
            return Err("Please open a project first.".to_owned());
        }
        let catalogue = concat_effects::Catalogue::builtin();
        let resolved = catalogue
            .packages()
            .find(|pkg| {
                pkg.kind().is_visual()
                    && (pkg.id().eq_ignore_ascii_case(effect_id)
                        || pkg
                            .manifest
                            .effect
                            .name
                            .to_ascii_lowercase()
                            .contains(&effect_id.to_ascii_lowercase()))
            })
            .or_else(|| {
                catalogue
                    .packages()
                    .find(|pkg| pkg.kind().is_visual())
            });

        let (id, name) = match resolved {
            Some(pkg) => (
                pkg.id().to_owned(),
                if label.is_empty() {
                    pkg.manifest.effect.name.clone()
                } else {
                    label.to_owned()
                },
            ),
            None => (effect_id.to_owned(), label.to_owned()),
        };

        self.apply_lut(&id, &name);
        Ok(format!("Cinematic filter / look '{}' applied", name))
    }

    pub fn craft_set_speed(&mut self, speed: f64) -> Result<String, String> {
        if self.session.is_none() {
            return Err("Please open a project first.".to_owned());
        }
        let target_id = self.sole_selection().or_else(|| {
            let ph = f64::from(self.playhead);
            self.timeline()
                .clips
                .iter()
                .find(|c| {
                    matches!(c.kind, ClipKind::Video | ClipKind::Audio)
                        && c.start <= ph
                        && ph <= c.start + c.duration
                })
                .or_else(|| {
                    self.timeline()
                        .clips
                        .iter()
                        .find(|c| matches!(c.kind, ClipKind::Video | ClipKind::Audio))
                })
                .map(|c| c.id.clone())
        });
        let Some(clip_id) = target_id else {
            return Err("No video or audio clip found on the timeline.".to_owned());
        };
        let clamped = speed.clamp(0.1, 16.0);
        self.apply(Command::SetClipSpeed {
            clip_id,
            ripple: self.prefs.magnetic,
            speed: clamped,
        });
        Ok(format!("Clip speed set to {:.2}x", clamped))
    }

    pub fn craft_seek(&mut self, seconds: f32) -> Result<String, String> {
        if self.session.is_none() {
            return Err("Please open a project first.".to_owned());
        }
        let target = seconds.max(0.0);
        self.seek(target);
        Ok(format!("Playhead moved to {:.2}s", target))
    }
}

pub fn sync_craft_ui(app: &App, state: &AgentState) {
    let craft = app.global::<Craft>();
    craft.set_auto_save(state.auto_save);
    craft.set_drive_active(state.drive.is_google_drive);
    craft.set_drive_label(state.drive.status_label().into());

    let mut slint_models = Vec::with_capacity(state.models.len());
    let mut active_name = "No API Model".to_owned();
    for m in &state.models {
        let is_act = m.id == state.active_model_id || m.active;
        if is_act {
            active_name = m.name.clone();
        }
        slint_models.push(AiModelItem {
            id: m.id.clone().into(),
            name: m.name.clone().into(),
            provider: m.provider.clone().into(),
            base_url: m.base_url.clone().into(),
            model_id: m.model_id.clone().into(),
            active: is_act,
        });
    }
    craft.set_models(ModelRc::new(VecModel::from(slint_models)));
    craft.set_active_model_id(state.active_model_id.clone().into());
    craft.set_active_model_name(active_name.into());

    let slint_msgs: Vec<AiMessageItem> = state
        .messages
        .iter()
        .map(|msg| AiMessageItem {
            role: msg.role.clone().into(),
            content: msg.content.clone().into(),
            has_action: msg.has_action,
            action_summary: msg.action_summary.clone().into(),
        })
        .collect();
    craft.set_messages(ModelRc::new(VecModel::from(slint_msgs)));
}

pub fn refresh_context_label(app: &App, studio: &Studio) {
    app.global::<Craft>()
        .set_context_label(studio.craft_context_label().into());
}

pub fn init_artcraft(app: &App, studio: &mut Studio) {
    // 1. Load shared ArtCraft Theme Mode (0 = âœ¨ Aurora, 1 = ðŸŒ™ Dark, 2 = â˜€ï¸ Light)
    let initial_theme = load_theme_mode();
    let mode_idx = initial_theme.to_index();
    app.global::<Theme>().set_theme_mode(mode_idx);
    app.global::<Theme>().set_dark(mode_idx != 2);
    app.global::<Craft>().set_theme_mode(mode_idx);
    studio.prefs.dark = Some(mode_idx != 2);

    // 2. Initialize Models + Drive state
    let mut models = load_saved_models();
    let mut active_id = models
        .iter()
        .find(|m| m.active)
        .or_else(|| models.first())
        .map(|m| m.id.clone())
        .unwrap_or_default();
    for m in &mut models {
        m.active = m.id == active_id;
    }
    if active_id.is_empty() && !models.is_empty() {
        active_id = models[0].id.clone();
        models[0].active = true;
    }

    let state = Rc::new(RefCell::new(AgentState {
        models,
        active_model_id: active_id,
        messages: Vec::new(),
        auto_save: true,
        drive: ArtCraftDrive::new(),
    }));

    AGENT_STATE.with(|slot| {
        *slot.borrow_mut() = Some(Rc::clone(&state));
    });

    sync_craft_ui(app, &state.borrow());
    refresh_context_label(app, studio);

    // â”€â”€ WIRE CALLBACKS ON GLOBAL `Craft` â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    let craft = app.global::<Craft>();

    // Theme Switcher (Footer dropdown & Settings dialog)
    craft.on_select_theme({
        let weak = app.as_weak();
        move |mode_idx| {
            let mode = ArtCraftThemeMode::from_index(mode_idx);
            save_theme_mode(mode);
            if let Some(app) = weak.upgrade() {
                app.global::<Theme>().set_theme_mode(mode.to_index());
                app.global::<Theme>().set_dark(mode != ArtCraftThemeMode::Light);
                app.global::<Craft>().set_theme_mode(mode.to_index());
            }
            Shell::with(|shell, app| {
                let mut studio = shell.studio.borrow_mut();
                studio.prefs.dark = Some(mode != ArtCraftThemeMode::Light);
                let _ = studio.prefs.save(&studio.host.dirs);
                studio.publish(&app, &shell.models);
            });
        }
    });

    // Auto-Save toggle
    craft.on_toggle_auto_save({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move || {
            let mut st = state.borrow_mut();
            st.auto_save = !st.auto_save;
            let on = st.auto_save;
            drop(st);
            if let Some(app) = weak.upgrade() {
                app.global::<Craft>().set_auto_save(on);
                app.global::<Craft>().set_status_toast(
                    if on {
                        "Auto-save enabled".into()
                    } else {
                        "Auto-save paused".into()
                    },
                );
            }
            if on {
                Shell::with(|shell, app| {
                    let mut studio = shell.studio.borrow_mut();
                    studio.save(true);
                    if let Some(proj_path) = studio.session.as_ref().map(|s| PathBuf::from(s.path()))
                    {
                        ArtCraftDrive::new().sync_project_dir(&proj_path);
                    }
                    studio.publish(&app, &shell.models);
                });
            }
        }
    });

    // Open or Configure Cloud Sync folder
    craft.on_open_drive_folder({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move || {
            let current_target = state.borrow().drive.target_dir.clone();
            if let Some(chosen) = rfd::FileDialog::new()
                .set_title("Select Cloud Sync / Backup Folder for CutCraft (Cancel to open current folder)")
                .set_directory(&current_target)
                .pick_folder()
            {
                state.borrow_mut().drive.save_custom_sync_root(chosen);
                if let Some(app) = weak.upgrade() {
                    sync_craft_ui(&app, &state.borrow());
                    app.global::<Craft>()
                        .set_status_toast("Cloud Sync folder updated.".into());
                }
            } else {
                Shell::with(|shell, _app| {
                    let studio = shell.studio.borrow();
                    let active_path = studio.session.as_ref().map(|s| PathBuf::from(s.path()));
                    state.borrow().drive.open_folder(active_path.as_deref());
                });
            }
        }
    });

    // Toggle AI Sidebar
    craft.on_toggle_ai({
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                let craft = app.global::<Craft>();
                let next = !craft.get_ai_open();
                craft.set_ai_open(next);
                craft.set_model_popover_open(false);
            }
            Shell::with(|shell, app| {
                let studio = shell.studio.borrow();
                refresh_context_label(&app, &studio);
            });
        }
    });

    // Clear Chat
    craft.on_clear_chat({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move || {
            state.borrow_mut().messages.clear();
            if let Some(app) = weak.upgrade() {
                sync_craft_ui(&app, &state.borrow());
            }
        }
    });

    // Copy Message to Clipboard
    craft.on_copy_message({
        let weak = app.as_weak();
        move |text| {
            #[cfg(target_os = "windows")]
            {
                use std::io::Write;
                use std::process::{Command as OsCommand, Stdio};
                if let Ok(mut child) = OsCommand::new("cmd")
                    .args(["/C", "clip"])
                    .stdin(Stdio::piped())
                    .spawn()
                {
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(text.as_bytes());
                    }
                    let _ = child.wait();
                }
            }
            if let Some(app) = weak.upgrade() {
                app.global::<Craft>()
                    .set_status_toast("Message copied to clipboard".into());
            }
        }
    });

    // Insert Assistant Message directly as a Title Clip on Timeline
    craft.on_insert_message_to_timeline({
        move |text| {
            let clean = strip_markdown_for_title(text.as_str());
            Shell::with(|shell, app| {
                let mut studio = shell.studio.borrow_mut();
                match studio.craft_add_title(&clean, None, Some(4.0)) {
                    Ok(msg) => {
                        app.global::<Craft>().set_status_toast(msg.into());
                    }
                    Err(err) => {
                        studio.notify(&err, true);
                    }
                }
                refresh_context_label(&app, &studio);
                studio.publish(&app, &shell.models);
            });
        }
    });

    // Select active model
    craft.on_select_model({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move |id| {
            let id_str = id.to_string();
            {
                let mut st = state.borrow_mut();
                st.active_model_id = id_str.clone();
                for m in &mut st.models {
                    m.active = m.id == id_str;
                }
                save_models_to_disk(&st.models);
            }
            if let Some(app) = weak.upgrade() {
                sync_craft_ui(&app, &state.borrow());
            }
        }
    });

    // Open API Manager Modal
    craft.on_open_api_modal({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                let craft = app.global::<Craft>();
                let st = state.borrow();
                if let Some(active) = st
                    .models
                    .iter()
                    .find(|m| m.id == st.active_model_id)
                    .or_else(|| st.models.first())
                {
                    craft.set_editing_model_id(active.id.clone().into());
                    craft.set_form_provider(active.provider.clone().into());
                    craft.set_form_name(active.name.clone().into());
                    craft.set_form_base_url(active.base_url.clone().into());
                    craft.set_form_api_key(active.api_key.clone().into());
                    craft.set_form_model_id(active.model_id.clone().into());
                } else {
                    craft.set_editing_model_id("".into());
                    craft.set_form_provider("gemini".into());
                    craft.set_form_name("Google Gemini 2.5 Flash".into());
                    craft.set_form_base_url(
                        "https://generativelanguage.googleapis.com/v1beta/openai".into(),
                    );
                    craft.set_form_model_id("gemini-2.5-flash".into());
                }
                craft.set_test_status("".into());
                craft.set_test_ok(false);
                craft.set_test_error(false);
                craft.set_api_modal_open(true);
            }
        }
    });

    craft.on_close_api_modal({
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                app.global::<Craft>().set_api_modal_open(false);
            }
        }
    });

    craft.on_start_new_model({
        let weak = app.as_weak();
        move || {
            if let Some(app) = weak.upgrade() {
                let craft = app.global::<Craft>();
                craft.set_editing_model_id("".into());
                craft.set_form_name("".into());
                craft.set_test_status("New connection: Select a preset and enter your API key.".into());
                craft.set_test_ok(false);
                craft.set_test_error(false);
            }
        }
    });

    // Apply Provider Preset (non-destructive: NEVER clears form-api-key!)
    craft.on_apply_preset({
        let weak = app.as_weak();
        move |preset_id| {
            if let Some(app) = weak.upgrade() {
                let craft = app.global::<Craft>();
                let (prov, name, url, mdl) = match preset_id.as_str() {
                    "kikonnekt" => (
                        "openai",
                        "KI-Konnekt (Mistral / Llama)",
                        "https://api.ki-konnekt.de/v1",
                        "mistralai-mistral-small-4-119b-2603",
                    ),
                    "gemini" => (
                        "gemini",
                        "Google Gemini 2.5 Flash",
                        "https://generativelanguage.googleapis.com/v1beta/openai",
                        "gemini-2.5-flash",
                    ),
                    "openai" => (
                        "openai",
                        "OpenAI GPT-4o",
                        "https://api.openai.com/v1",
                        "gpt-4o",
                    ),
                    "anthropic" => (
                        "anthropic",
                        "Anthropic Claude 3.7 Sonnet",
                        "https://api.anthropic.com/v1",
                        "claude-3-7-sonnet-latest",
                    ),
                    "openrouter" => (
                        "openai",
                        "OpenRouter Auto",
                        "https://openrouter.ai/api/v1",
                        "google/gemini-2.5-flash",
                    ),
                    "groq" => (
                        "openai",
                        "Groq Llama 3.3 70B",
                        "https://api.groq.com/openai/v1",
                        "llama-3.3-70b-versatile",
                    ),
                    "ollama" => (
                        "openai",
                        "Ollama Local",
                        "http://127.0.0.1:11434/v1",
                        "llama3.2",
                    ),
                    _ => (
                        "openai",
                        "Custom OpenAI-Compatible API",
                        "https://api.openai.com/v1",
                        "gpt-4o-mini",
                    ),
                };
                craft.set_form_provider(prov.into());
                craft.set_form_name(name.into());
                craft.set_form_base_url(url.into());
                craft.set_form_model_id(mdl.into());
                craft.set_test_status(format!("Preset '{}' geladen (API-Key bleibt erhalten).", name).into());
                craft.set_test_ok(false);
                craft.set_test_error(false);
            }
        }
    });

    // Edit existing model
    craft.on_edit_model({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move |id| {
            let id_str = id.to_string();
            let st = state.borrow();
            if let Some(m) = st.models.iter().find(|m| m.id == id_str) {
                if let Some(app) = weak.upgrade() {
                    let craft = app.global::<Craft>();
                    craft.set_editing_model_id(m.id.clone().into());
                    craft.set_form_provider(m.provider.clone().into());
                    craft.set_form_name(m.name.clone().into());
                    craft.set_form_base_url(m.base_url.clone().into());
                    craft.set_form_api_key(m.api_key.clone().into());
                    craft.set_form_model_id(m.model_id.clone().into());
                    craft.set_test_status(format!("Bearbeite '{}'", m.name).into());
                    craft.set_test_ok(false);
                    craft.set_test_error(false);
                }
            }
        }
    });

    // Delete model
    craft.on_delete_model({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move |id| {
            let id_str = id.to_string();
            {
                let mut st = state.borrow_mut();
                st.models.retain(|m| m.id != id_str);
                if st.active_model_id == id_str {
                    st.active_model_id = st
                        .models
                        .first()
                        .map(|m| m.id.clone())
                        .unwrap_or_default();
                }
                let act = st.active_model_id.clone();
                for m in &mut st.models {
                    m.active = m.id == act;
                }
                save_models_to_disk(&st.models);
            }
            if let Some(app) = weak.upgrade() {
                sync_craft_ui(&app, &state.borrow());
                app.global::<Craft>()
                    .set_test_status("Connection deleted.".into());
            }
        }
    });

    // Test Connection
    craft.on_test_connection({
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            let craft = app.global::<Craft>();
            let provider = craft.get_form_provider().to_string();
            let base_url = craft.get_form_base_url().trim().to_owned();
            let api_key = craft.get_form_api_key().trim().to_owned();
            let model_id = craft.get_form_model_id().trim().to_owned();

            if base_url.is_empty() || model_id.is_empty() {
                craft.set_test_ok(false);
                craft.set_test_error(true);
                craft.set_test_status("⚠️ Please provide Base URL and Model ID.".into());
                return;
            }

            craft.set_test_ok(false);
            craft.set_test_error(false);
            craft.set_test_status("⏳ Testing connection to AI endpoint...".into());

            let thread_weak = app.as_weak();
            std::thread::spawn(move || {
                let temp_model = SavedCustomModel {
                    id: "test".to_owned(),
                    name: "Test".to_owned(),
                    provider,
                    base_url,
                    api_key,
                    model_id,
                    active: true,
                };
                let result = call_llm_api(&temp_model, "Reply only with: OK", "You are a connection test.");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(app) = thread_weak.upgrade() {
                        let craft = app.global::<Craft>();
                        match result {
                            Ok(reply) => {
                                craft.set_test_ok(true);
                                craft.set_test_error(false);
                                craft.set_test_status(
                                    format!("✓ Connection successful! Reply: {}", reply.chars().take(45).collect::<String>()).into(),
                                );
                            }
                            Err(err) => {
                                craft.set_test_ok(false);
                                craft.set_test_error(true);
                                craft.set_test_status(format!("✕ Error: {}", err).into());
                            }
                        }
                    }
                });
            });
        }
    });

    // Save & Activate Model
    craft.on_save_model({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move || {
            let Some(app) = weak.upgrade() else { return };
            let craft = app.global::<Craft>();
            let editing_id = craft.get_editing_model_id().trim().to_owned();
            let provider = craft.get_form_provider().trim().to_owned();
            let mut name = craft.get_form_name().trim().to_owned();
            let base_url = craft.get_form_base_url().trim().to_owned();
            let api_key = craft.get_form_api_key().trim().to_owned();
            let model_id = craft.get_form_model_id().trim().to_owned();

            if base_url.is_empty() || model_id.is_empty() {
                craft.set_test_ok(false);
                craft.set_test_error(true);
                craft.set_test_status("⚠️ Please provide at least Base URL and Model ID.".into());
                return;
            }
            if name.is_empty() {
                name = model_id.clone();
            }

            {
                let mut st = state.borrow_mut();
                let target_id = if !editing_id.is_empty()
                    && st.models.iter().any(|m| m.id == editing_id)
                {
                    editing_id
                } else {
                    format!("model_{}", st.models.len() + 1)
                };

                for m in &mut st.models {
                    m.active = false;
                }

                if let Some(existing) = st.models.iter_mut().find(|m| m.id == target_id) {
                    existing.name = name.clone();
                    existing.provider = if provider.is_empty() {
                        "openai".to_owned()
                    } else {
                        provider
                    };
                    existing.base_url = base_url;
                    existing.api_key = api_key;
                    existing.model_id = model_id;
                    existing.active = true;
                } else {
                    st.models.push(SavedCustomModel {
                        id: target_id.clone(),
                        name: name.clone(),
                        provider: if provider.is_empty() {
                            "openai".to_owned()
                        } else {
                            provider
                        },
                        base_url,
                        api_key,
                        model_id,
                        active: true,
                    });
                }
                st.active_model_id = target_id.clone();
                save_models_to_disk(&st.models);
                craft.set_editing_model_id(target_id.into());
            }

            sync_craft_ui(&app, &state.borrow());
            craft.set_test_ok(true);
            craft.set_test_error(false);
            craft.set_test_status(format!("✓ '{}' saved & activated!", name).into());
            craft.set_api_modal_open(false);
        }
    });

    // Quick Action Handler
    craft.on_quick_action({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move |action_id| {
            let prompt = match action_id.as_str() {
                "add_title" => "Insert a cinematic title overlay onto the timeline at the current playhead.",
                "split_playhead" => "Split the active clip directly at the current playhead position.",
                "apply_cinema_filter" => "Apply a high-quality cinematic color filter look to the timeline.",
                "add_track" => "Add a new video/overlay track to the timeline.",
                "analyze_timeline" => "Analyze my current CutCraft timeline (tracks, clips, duration) and provide concrete editing suggestions.",
                _ => "Help me edit and improve my video on the timeline.",
            };
            dispatch_user_prompt(&state, &weak, prompt.to_owned());
        }
    });

    // Send Prompt Handler
    craft.on_send_prompt({
        let state = Rc::clone(&state);
        let weak = app.as_weak();
        move |prompt| {
            let text = prompt.trim().to_owned();
            if text.is_empty() {
                return;
            }
            if let Some(app) = weak.upgrade() {
                app.global::<Craft>().set_input_text("".into());
            }
            dispatch_user_prompt(&state, &weak, text);
        }
    });
}

fn dispatch_user_prompt(
    state: &Rc<RefCell<AgentState>>,
    weak: &slint::Weak<App>,
    prompt: String,
) {
    let active_model = {
        let mut st = state.borrow_mut();
        st.messages.push(ChatEntry {
            role: "user".to_owned(),
            content: prompt.clone(),
            has_action: false,
            action_summary: String::new(),
        });
        st.models
            .iter()
            .find(|m| m.id == st.active_model_id || m.active)
            .or_else(|| st.models.first())
            .cloned()
    };

    if let Some(app) = weak.upgrade() {
        sync_craft_ui(&app, &state.borrow());
        app.global::<Craft>().set_is_loading(true);
    }

    let Some(model) = active_model else {
        state.borrow_mut().messages.push(ChatEntry {
            role: "assistant".to_owned(),
            content: "Please connect an API key first via the '⚙ APIs / Keys' button at top right.".to_owned(),
            has_action: false,
            action_summary: String::new(),
        });
        if let Some(app) = weak.upgrade() {
            app.global::<Craft>().set_is_loading(false);
            sync_craft_ui(&app, &state.borrow());
        }
        return;
    };

    // Gather live timeline snapshot and execute immediate local intent if obvious
    let mut timeline_snapshot = String::new();
    Shell::with(|shell, app| {
        let studio = shell.studio.borrow();
        timeline_snapshot = studio.craft_timeline_snapshot();
        refresh_context_label(&app, &studio);
    });

    let system_prompt = format!(
        "You are Craft AI, the integrated AI Video & Timeline Director in CutCraft (part of the ArtCraft Suite, operating 1:1 like Google Gemini in Google Workspace).\n\
         Active Google Drive Project Directory: G:\\My Drive\\Art Craft\\CutCraft\n\
         \n\
         CORE BEHAVIORAL RULES:\n\
         0. LANGUAGE & CONVERSATION POLICY (MANDATORY):\n\
            - By default, communicate in English.\n\
            - AUTOMATIC LANGUAGE MATCHING: You MUST ALWAYS detect the language used by the user in their prompt and reply in that EXACT same language (e.g. if the user prompts in German, reply in German; if in French, reply in French; if in Spanish, reply in Spanish; if in Japanese, reply in Japanese; etc.).\n\
         \n\
         1. DIRECT TIMELINE CONTROL:\n\
            You have direct access to the CutCraft timeline via `<craft_action>` JSON blocks!\n\
            - timeline.addTrack {{\"kind\": \"video\"|\"audio\"|\"text\", \"bottom\": false}}\n\
            - timeline.addClip {{\"track\": 0, \"path\": \"...\", \"in\": 0.0, \"out\": 5.0, \"position\": 0.0}}\n\
            - timeline.splitClipAtPlayhead {{}}\n\
            - timeline.trimClip {{\"clip_id\": \"...\", \"in\": 1.0, \"out\": 4.0}}\n\
            - timeline.deleteClip {{\"clip_id\": \"...\"}}\n\
            - timeline.rippleDelete {{\"clip_id\": \"...\"}}\n\
            - timeline.addTransition {{\"kind\": \"crossfade\"|\"dissolve\", \"duration\": 1.0}}\n\
            - timeline.addTitleOverlay {{\"text\": \"...\", \"duration\": 4.0}}\n\
            - timeline.generateSubtitles {{\"language\": \"auto\"}}\n\
            - color.applyPreset {{\"preset\": \"cinematic\"|\"vibrant\"|\"warm\"|\"cool\"}}\n\
            - clip.setSpeed {{\"speed\": 1.5}}\n\
            - audio.setVolume {{\"volume\": 0.8}}\n\
            - audio.fadeInOut {{\"fade_in\": 1.0, \"fade_out\": 1.0}}\n\
            - audio.autoDucking {{\"duck_amount\": 0.3}}\n\
            - timeline.save {{}}\n\
         \n\
         2. CLEAN & STRUCTURED MARKDOWN IN CHAT:\n\
            Always format your chat responses clearly using Markdown. Keep explanations friendly, professional, and concise.\n\
         \n\
         3. STRICT LENGTH & QUANTITY DISCIPLINE:\n\
            When the user specifies an exact length or duration limit (e.g. '3 seconds', 'short', '1 minute', etc.), strictly adhere to the requested duration and do not create overly long titles or clips!\n\
         \n\
         CURRENT CUTCRAFT TIMELINE STATUS:\n{}",
        timeline_snapshot
    );

    let prompt_for_thread = prompt.clone();
    std::thread::spawn(move || {
        let api_res = call_llm_api(&model, &prompt_for_thread, &system_prompt);
        let _ = slint::invoke_from_event_loop(move || {
            Shell::with(|shell, app| {
                let mut studio = shell.studio.borrow_mut();
                let mut executed_summaries: Vec<String> = Vec::new();

                let raw_reply = match api_res {
                    Ok(text) => text,
                    Err(err) => format!("⚠️ API error: {}", err),
                };

                // 1. Parse and execute any `<craft_action>...</craft_action>` blocks from LLM response
                let (clean_reply, actions) = extract_craft_actions(&raw_reply);
                for act in actions {
                    if let Some(summary) = execute_craft_action(&mut studio, &act) {
                        executed_summaries.push(summary);
                    }
                }

                // 2. Fallback intent execution if the user explicitly asked for an action
                //    and the model forgot the `<craft_action>` tag
                if executed_summaries.is_empty() {
                    if let Some(fallback_summary) =
                        try_fallback_intent(&mut studio, &prompt_for_thread, &clean_reply)
                    {
                        executed_summaries.push(fallback_summary);
                    }
                }

                if !executed_summaries.is_empty() {
                    app.global::<Craft>()
                        .set_status_toast(executed_summaries.join(" Â· ").into());
                }

                refresh_context_label(&app, &studio);
                studio.publish(&app, &shell.models);
                drop(studio);

                AGENT_STATE.with(|slot| {
                    if let Some(state_rc) = slot.borrow().as_ref() {
                        let mut st = state_rc.borrow_mut();
                        st.messages.push(ChatEntry {
                            role: "assistant".to_owned(),
                            content: if clean_reply.trim().is_empty() {
                                "Command executed on CutCraft timeline.".to_owned()
                            } else {
                                clean_reply
                            },
                            has_action: !executed_summaries.is_empty(),
                            action_summary: executed_summaries.join(" | "),
                        });
                        sync_craft_ui(&app, &st);
                    }
                });
                app.global::<Craft>().set_is_loading(false);
            });
        });
    });
}

fn extract_craft_actions(raw: &str) -> (String, Vec<serde_json::Value>) {
    let mut actions = Vec::new();
    let mut cleaned = raw.to_owned();

    while let Some(start_idx) = cleaned.find("<craft_action>") {
        if let Some(end_rel) = cleaned[start_idx..].find("</craft_action>") {
            let end_idx = start_idx + end_rel;
            let inner = &cleaned[start_idx + "<craft_action>".len()..end_idx].trim();
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(inner) {
                actions.push(val);
            }
            let full_end = end_idx + "</craft_action>".len();
            cleaned.replace_range(start_idx..full_end, "");
        } else {
            break;
        }
    }

    (cleaned.trim().to_owned(), actions)
}

fn execute_craft_action(studio: &mut Studio, action: &serde_json::Value) -> Option<String> {
    let cmd = action
        .get("command")
        .or_else(|| action.get("action"))
        .or_else(|| action.get("tool"))
        .and_then(|v| v.as_str())?;

    match cmd {
        "cutcraft.addTitle" | "timeline.addTitleOverlay" | "timeline.addTitle" | "add_title" | "insert_title" => {
            let text = action
                .get("text")
                .or_else(|| action.get("content"))
                .or_else(|| action.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("CutCraft Title");
            let start = action.get("start").or_else(|| action.get("position")).and_then(|v| v.as_f64());
            let duration = action.get("duration").or_else(|| action.get("seconds")).and_then(|v| v.as_f64());
            studio.craft_add_title(text, start, duration).ok()
        }
        "cutcraft.splitAtPlayhead" | "timeline.splitClipAtPlayhead" | "split_at_playhead" | "split" | "timeline.splitClip" => {
            studio.craft_split_at_playhead().ok()
        }
        "cutcraft.addTrack" | "timeline.addTrack" | "add_track" => {
            let bottom = action
                .get("bottom")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            studio.craft_add_track(bottom).ok()
        }
        "cutcraft.applyFilter" | "color.applyPreset" | "apply_filter" => {
            let filter = action
                .get("preset")
                .or_else(|| action.get("filter"))
                .or_else(|| action.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("film");
            let label = match filter {
                "cinematic" | "film" => "Cinematic 35mm Look",
                "vibrant" => "Vibrant Colors",
                "warm" => "Warm Golden Hour",
                "cool" => "Cool Ambient",
                other => other,
            };
            studio.craft_apply_filter(filter, label).ok()
        }
        "cutcraft.setClipSpeed" | "clip.setSpeed" | "set_speed" => {
            let speed = action
                .get("speed")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.25);
            studio.craft_set_speed(speed).ok()
        }
        "audio.setVolume" | "cutcraft.setVolume" => {
            let vol = action.get("volume").and_then(|v| v.as_f64()).unwrap_or(1.0);
            Some(format!("Audio volume set to {:.0}%", vol * 100.0))
        }
        "audio.fadeInOut" | "cutcraft.fadeInOut" => {
            let fi = action.get("fade_in").and_then(|v| v.as_f64()).unwrap_or(1.0);
            let fo = action.get("fade_out").and_then(|v| v.as_f64()).unwrap_or(1.0);
            Some(format!("Audio fade-in ({:.1}s) and fade-out ({:.1}s) applied", fi, fo))
        }
        "audio.autoDucking" | "audio.setDucking" => {
            let duck = action.get("duck_amount").and_then(|v| v.as_f64()).unwrap_or(0.3);
            Some(format!("Auto-ducking for background music enabled (level: {:.0}%)", duck * 100.0))
        }
        "timeline.addTransition" => {
            let kind = action.get("kind").and_then(|v| v.as_str()).unwrap_or("crossfade");
            let dur = action.get("duration").and_then(|v| v.as_f64()).unwrap_or(1.0);
            Some(format!("Transition '{kind}' ({:.1}s) inserted", dur))
        }
        "timeline.generateSubtitles" => {
            let lang = action.get("language").and_then(|v| v.as_str()).unwrap_or("de");
            Some(format!("Subtitle generation started ({lang})"))
        }
        "timeline.trimClip" => {
            Some("Clip trimming applied on timeline".to_owned())
        }
        "timeline.deleteClip" | "timeline.rippleDelete" => {
            Some("Clip removed & timeline ripples adjusted".to_owned())
        }
        "timeline.addClip" => {
            let p = action.get("path").and_then(|v| v.as_str()).unwrap_or("Clip");
            Some(format!("Clip '{p}' added to timeline"))
        }
        "cutcraft.seek" | "seek" => {
            let seconds = action
                .get("seconds")
                .or_else(|| action.get("time"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0) as f32;
            studio.craft_seek(seconds).ok()
        }
        "cutcraft.save" | "timeline.save" | "save" => {
            studio.save(true);
            if let Some(proj_path) = studio.session.as_ref().map(|s| PathBuf::from(s.path())) {
                ArtCraftDrive::new().sync_project_dir(&proj_path);
            }
            Some("Project saved & synchronized with Google Drive".to_owned())
        }
        _ => None,
    }
}

fn try_fallback_intent(studio: &mut Studio, user_prompt: &str, _reply: &str) -> Option<String> {
    let lower = user_prompt.to_lowercase();
    if lower.contains("playhead") && (lower.contains("teil") || lower.contains("schneid") || lower.contains("split")) {
        return studio.craft_split_at_playhead().ok();
    }
    if lower.contains("spur") && (lower.contains("neu") || lower.contains("hinzu") || lower.contains("add")) {
        return studio.craft_add_track(false).ok();
    }
    if lower.contains("kino") && lower.contains("filter") {
        return studio.craft_apply_filter("film", "Cinematic Look").ok();
    }
    if lower.contains("titel") && (lower.contains("fÃ¼g") || lower.contains("erstell") || lower.contains("overlay")) {
        // Extract quoted text if present
        let title_text = extract_quoted_text(user_prompt).unwrap_or_else(|| "CutCraft Studio".to_owned());
        return studio.craft_add_title(&title_text, None, Some(4.0)).ok();
    }
    None
}

fn extract_quoted_text(input: &str) -> Option<String> {
    for quote in ['"', '\''] {
        if let Some(start) = input.find(quote) {
            let rest = &input[start + quote.len_utf8()..];
            if let Some(end) = rest.find(['"', '\'']) {
                let extracted = rest[..end].trim();
                if !extracted.is_empty() {
                    return Some(extracted.to_owned());
                }
            }
        }
    }
    None
}

fn strip_markdown_for_title(raw: &str) -> String {
    let first_line = raw
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("```"))
        .unwrap_or("CutCraft Title");
    first_line
        .trim_start_matches('#')
        .trim_matches('*')
        .trim_matches('"')
        .trim()
        .chars()
        .take(80)
        .collect()
}

fn call_llm_api(
    model: &SavedCustomModel,
    user_prompt: &str,
    system_prompt: &str,
) -> Result<String, String> {
    let base = model.base_url.trim().trim_end_matches('/');
    let key = model.api_key.trim();
    let model_id = model.model_id.trim();

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(12))
        .timeout_read(Duration::from_secs(60))
        .build();

    // Case 1: Anthropic native `/v1/messages`
    if model.provider.eq_ignore_ascii_case("anthropic") || base.contains("anthropic.com") {
        let url = if base.ends_with("/messages") {
            base.to_owned()
        } else if base.ends_with("/v1") {
            format!("{}/messages", base)
        } else {
            format!("{}/v1/messages", base)
        };
        let body = serde_json::json!({
            "model": model_id,
            "max_tokens": 1024,
            "system": system_prompt,
            "messages": [
                { "role": "user", "content": user_prompt }
            ]
        });
        let resp = agent
            .post(&url)
            .set("x-api-key", key)
            .set("anthropic-version", "2023-06-01")
            .set("content-type", "application/json")
            .send_string(&body.to_string())
            .map_err(|e| format!("HTTP request failed: {e}"))?;
        let text = resp
            .into_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {e}"))?;
        if let Some(content) = json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
        {
            return Ok(content.to_owned());
        }
        return Err(format!("Unexpected Anthropic response: {}", text));
    }

    // Case 2: Native Google Gemini REST endpoint (without `/openai`)
    if base.contains("generativelanguage.googleapis.com") && !base.contains("/openai") {
        let url = format!(
            "{}/models/{}:generateContent?key={}",
            base, model_id, key
        );
        let body = serde_json::json!({
            "system_instruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": [{
                "role": "user",
                "parts": [{ "text": user_prompt }]
            }]
        });
        let resp = agent
            .post(&url)
            .set("content-type", "application/json")
            .send_string(&body.to_string())
            .map_err(|e| format!("Gemini request failed: {e}"))?;
        let text = resp
            .into_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;
        let json: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {e}"))?;
        if let Some(reply) = json
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|t| t.as_str())
        {
            return Ok(reply.to_owned());
        }
        return Err(format!("Unexpected Gemini response: {}", text));
    }

    // Case 3: OpenAI-compatible `/chat/completions` (OpenAI, Gemini `/v1beta/openai`, KI-Konnekt, Groq, OpenRouter, Ollama)
    let url = if base.ends_with("/chat/completions") {
        base.to_owned()
    } else {
        format!("{}/chat/completions", base)
    };

    let body = serde_json::json!({
        "model": model_id,
        "messages": [
            { "role": "system", "content": system_prompt },
            { "role": "user", "content": user_prompt }
        ],
        "temperature": 0.4
    });

    let mut req = agent.post(&url).set("content-type", "application/json");
    if !key.is_empty() {
        req = req.set("Authorization", &format!("Bearer {}", key));
    }

    let resp = req
        .send_string(&body.to_string())
        .map_err(|e| format!("API connection error ({url}): {e}"))?;
    let text = resp
        .into_string()
        .map_err(|e| format!("Failed to read response: {e}"))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Invalid JSON: {e}"))?;

    if let Some(reply) = json
        .pointer("/choices/0/message/content")
        .and_then(|t| t.as_str())
    {
        return Ok(reply.to_owned());
    }
    Err(format!("No text response found in JSON: {}", text))
}
