use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

const POPUP_LABEL: &str = "main";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";
const DEFAULT_SHORTCUT: &str = "Ctrl+Alt+L";

// ── Position ─────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Position {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
    Center,
}

// ── Config ───────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default, Clone)]
struct AppConfig {
    /// Session cookie copied from claude.ai (sessionKey value).
    session_key: Option<String>,
    /// Cached organization UUID — auto-discovered on first call.
    org_id: Option<String>,
    /// Where on the primary monitor the popup appears. Default: bottom-right.
    #[serde(default)]
    position: Option<Position>,
    /// Global shortcut in Electron-accelerator format (e.g. "Ctrl+Alt+L").
    /// None = use the built-in default. Doesn't fire on Wayland.
    #[serde(default)]
    shortcut: Option<String>,
}

// Manual Debug to keep session_key out of any accidental log output.
impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field(
                "session_key",
                &self.session_key.as_ref().map(|_| "<redacted>"),
            )
            .field("org_id", &self.org_id)
            .field("position", &self.position)
            .field("shortcut", &self.shortcut)
            .finish()
    }
}

fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("claude-hourglass"))
}

fn config_path() -> Option<PathBuf> {
    Some(config_dir()?.join("config.json"))
}

/// One-time migration from the pre-rename config directory.
fn migrate_legacy_config() {
    let Some(new_path) = config_path() else {
        return;
    };
    if new_path.exists() {
        return;
    }
    let Some(new_dir) = config_dir() else {
        return;
    };
    let Some(parent) = new_dir.parent() else {
        return;
    };
    let legacy = parent.join("claude-usage-tray").join("config.json");
    if !legacy.exists() {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&new_dir) {
        eprintln!("[claude-hourglass] migration: mkdir failed: {e}");
        return;
    }
    match std::fs::rename(&legacy, &new_path) {
        Ok(()) => eprintln!(
            "[claude-hourglass] migrated config: {} -> {}",
            legacy.display(),
            new_path.display()
        ),
        Err(e) => eprintln!("[claude-hourglass] migration: rename failed: {e}"),
    }
}

fn read_config() -> AppConfig {
    migrate_legacy_config();
    config_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_config(cfg: &AppConfig) -> Result<(), String> {
    let path = config_path().ok_or("could not resolve config path")?;
    let parent = path
        .parent()
        .ok_or_else(|| "config path has no parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("mkdir failed: {e}"))?;
    let body = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;

    // Atomic write: tmp file with mode 0600 from creation, then rename.
    let tmp = parent.join(".config.json.tmp");

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| format!("open tmp failed: {e}"))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("write tmp failed: {e}"))?;
        f.sync_all().map_err(|e| format!("fsync failed: {e}"))?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&tmp, body).map_err(|e| format!("write tmp failed: {e}"))?;
    }

    std::fs::rename(&tmp, &path).map_err(|e| format!("rename failed: {e}"))
}

// ── Shortcut helpers ─────────────────────────────────────────────

fn parse_shortcut(s: &str) -> Result<Shortcut, String> {
    let sc = Shortcut::from_str(s).map_err(|e| format!("invalid shortcut '{s}': {e}"))?;
    if sc.mods.is_empty() {
        return Err(format!(
            "shortcut '{s}' must include at least one modifier (Ctrl/Alt/Shift/Cmd)"
        ));
    }
    Ok(sc)
}

/// Whether the in-app global-shortcut plugin can actually deliver events
/// on the current platform/session. Wayland blocks XGrabKey, so on Linux
/// Wayland sessions this returns false even though `register()` succeeds.
fn shortcut_supported() -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return false;
    }
    // Some compositors (nested setups, certain SSH-forwarded sessions) set
    // XDG_SESSION_TYPE=wayland but not WAYLAND_DISPLAY until first connection.
    // Use starts_with so variants like "wayland-gnome" are also caught.
    if std::env::var("XDG_SESSION_TYPE")
        .map(|v| v.to_ascii_lowercase().starts_with("wayland"))
        .unwrap_or(false)
    {
        return false;
    }
    true
}

// ── Usage report (sent to frontend) ──────────────────────────────

#[derive(Serialize, Default, Clone, Debug)]
struct UsageReport {
    available: bool,
    needs_setup: bool,
    error: Option<String>,
    five_hour_pct: Option<f64>,
    five_hour_reset_at: Option<String>,
    org_id: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
struct ConfigSummary {
    session_key: Option<String>,
    org_id: Option<String>,
    position: Position,
    shortcut: String,
    shortcut_supported: bool,
}

// ── claude.ai API ────────────────────────────────────────────────

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("http client init failed: {e}"))
}

async fn fetch_org_id(client: &reqwest::Client, session_key: &str) -> Result<String, String> {
    let resp = client
        .get("https://claude.ai/api/organizations")
        .header("Cookie", format!("sessionKey={session_key}"))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "/api/organizations -> {status}: {}",
            body.chars().take(200).collect::<String>()
        ));
    }
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("json parse: {e}"))?;
    let arr = body
        .as_array()
        .ok_or_else(|| "expected JSON array of organizations".to_string())?;
    arr.iter()
        .find_map(|o| o.get("uuid").and_then(|v| v.as_str()).map(String::from))
        .or_else(|| {
            arr.iter()
                .find_map(|o| o.get("id").and_then(|v| v.as_str()).map(String::from))
        })
        .ok_or_else(|| "no organization with uuid/id found".to_string())
}

async fn fetch_usage_json(
    client: &reqwest::Client,
    session_key: &str,
    org_id: &str,
) -> Result<serde_json::Value, String> {
    let url = format!("https://claude.ai/api/organizations/{org_id}/usage");
    let resp = client
        .get(&url)
        .header("Cookie", format!("sessionKey={session_key}"))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;
    let status = resp.status();
    if status.is_redirection()
        || status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        return Err("session_key invalid or expired — paste a fresh one".into());
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!(
            "/api/.../usage -> {status}: {}",
            body.chars().take(200).collect::<String>()
        ));
    }
    resp.json().await.map_err(|e| format!("json parse: {e}"))
}

fn pct_from(v: Option<&serde_json::Value>) -> Option<f64> {
    let obj = v?;
    obj.get("utilization_pct")
        .or_else(|| obj.get("utilization"))
        .and_then(|x| x.as_f64())
}

fn parse_usage_json(v: &serde_json::Value, org_id: String) -> UsageReport {
    let five = v.get("five_hour");
    let reset_at = five
        .and_then(|f| f.get("reset_at").or_else(|| f.get("resets_at")))
        .and_then(|x| x.as_str())
        .map(String::from);
    UsageReport {
        available: true,
        needs_setup: false,
        error: None,
        five_hour_pct: pct_from(five),
        five_hour_reset_at: reset_at,
        org_id: Some(org_id),
    }
}

// ── Tauri commands ───────────────────────────────────────────────

#[tauri::command]
async fn get_usage() -> Result<UsageReport, String> {
    let cfg = read_config();
    let Some(session_key) = cfg.session_key.clone() else {
        return Ok(UsageReport {
            available: false,
            needs_setup: true,
            ..Default::default()
        });
    };

    let client = build_client()?;

    let org_id = if let Some(id) = cfg.org_id.clone() {
        id
    } else {
        match fetch_org_id(&client, &session_key).await {
            Ok(id) => {
                let mut latest = read_config();
                if latest.session_key.as_deref() == Some(session_key.as_str()) {
                    latest.org_id = Some(id.clone());
                    let _ = write_config(&latest);
                }
                id
            }
            Err(e) => {
                return Ok(UsageReport {
                    available: false,
                    error: Some(format!("org discovery: {e}")),
                    ..Default::default()
                });
            }
        }
    };

    match fetch_usage_json(&client, &session_key, &org_id).await {
        Ok(json) => Ok(parse_usage_json(&json, org_id)),
        Err(e) => Ok(UsageReport {
            available: false,
            error: Some(e),
            org_id: Some(org_id),
            ..Default::default()
        }),
    }
}

#[tauri::command]
fn get_config() -> ConfigSummary {
    let cfg = read_config();
    ConfigSummary {
        session_key: cfg.session_key.as_ref().map(|_| "***".to_string()),
        org_id: cfg.org_id,
        position: cfg.position.unwrap_or_default(),
        shortcut: cfg
            .shortcut
            .clone()
            .unwrap_or_else(|| DEFAULT_SHORTCUT.to_string()),
        shortcut_supported: shortcut_supported(),
    }
}

#[tauri::command]
fn set_session_key(key: Option<String>) -> Result<(), String> {
    let mut cfg = read_config();
    cfg.session_key = key.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    cfg.org_id = None; // force rediscovery next call
    write_config(&cfg)
}

#[tauri::command]
fn set_position(app: AppHandle, position: Position) -> Result<(), String> {
    let mut cfg = read_config();
    cfg.position = Some(position);
    write_config(&cfg)?;
    // Apply immediately if the popup is currently visible.
    if let Some(window) = app.get_webview_window(POPUP_LABEL) {
        let _ = position_window(&window, position);
    }
    Ok(())
}

#[tauri::command]
fn set_shortcut(app: AppHandle, shortcut: String) -> Result<(), String> {
    // Validate first so we don't lose the working old shortcut on a typo.
    let new_shortcut = parse_shortcut(&shortcut)?;

    // Capture old shortcut so we can roll back if registration of the new
    // one fails (e.g. another app already grabbed that combo).
    let old_str = read_config().shortcut.clone();
    let old_shortcut = old_str.as_deref().and_then(|s| parse_shortcut(s).ok());

    let gs = app.global_shortcut();
    gs.unregister_all()
        .map_err(|e| format!("unregister: {e}"))?;

    if let Err(e) = gs.register(new_shortcut) {
        // Roll back to the old shortcut so the user isn't stuck with nothing.
        if let Some(old) = old_shortcut {
            if let Err(rollback_err) = gs.register(old) {
                eprintln!(
                    "[claude-hourglass] new shortcut register failed AND rollback failed: {rollback_err}"
                );
            }
        }
        return Err(format!("register: {e}"));
    }

    let mut cfg = read_config();
    cfg.shortcut = Some(shortcut);
    write_config(&cfg)
}

// ── Window helpers ───────────────────────────────────────────────

fn toggle_popup(app: &AppHandle) {
    let Some(window) = app.get_webview_window(POPUP_LABEL) else {
        return;
    };
    match window.is_visible() {
        Ok(true) => {
            // Let the frontend run its fade-out animation; it calls
            // window.hide() itself when the transition finishes.
            let _ = app.emit("popup-hide", ());
        }
        _ => {
            let pos = read_config().position.unwrap_or_default();
            let _ = position_window(&window, pos);
            let _ = window.show();
            let _ = window.set_focus();
            let _ = app.emit("popup-shown", ());
        }
    }
}

fn position_window(window: &tauri::WebviewWindow, pos: Position) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let size = monitor.size();
    let win_size = window.outer_size()?;
    let margin = (16.0 * scale) as i32;

    let w = size.width as i32;
    let h = size.height as i32;
    let ww = win_size.width as i32;
    let wh = win_size.height as i32;

    let (x, y) = match pos {
        Position::TopLeft => (margin, margin),
        Position::TopRight => (w - ww - margin, margin),
        Position::BottomLeft => (margin, h - wh - margin),
        Position::BottomRight => (w - ww - margin, h - wh - margin),
        Position::Center => ((w - ww) / 2, (h - wh) / 2),
    };
    window.set_position(tauri::PhysicalPosition::new(x, y))
}

// ── Entry ────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            eprintln!("[claude-hourglass] second instance args: {:?}", args);
            if args.iter().any(|a| a == "--toggle") {
                toggle_popup(app);
            } else if args.iter().any(|a| a == "--show") {
                if let Some(window) = app.get_webview_window(POPUP_LABEL) {
                    let pos = read_config().position.unwrap_or_default();
                    let _ = position_window(&window, pos);
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = app.emit("popup-shown", ());
                }
            } else if args.iter().any(|a| a == "--hide") {
                // Frontend-driven fade-out, same as toggle.
                let _ = app.emit("popup-hide", ());
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                // We register exactly one shortcut at a time, so any pressed
                // event is the configured one — no need to compare modifiers.
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        toggle_popup(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let handle = app.handle();

            // Register the configured (or default) global shortcut. This
            // succeeds on all platforms but only delivers events on
            // macOS, Windows, and Linux/X11. Wayland users bind via DE.
            let cfg = read_config();
            let shortcut_str = cfg
                .shortcut
                .clone()
                .unwrap_or_else(|| DEFAULT_SHORTCUT.to_string());
            match parse_shortcut(&shortcut_str) {
                Ok(s) => {
                    if let Err(e) = handle.global_shortcut().register(s) {
                        eprintln!(
                            "[claude-hourglass] register {shortcut_str} failed: {e}"
                        );
                    } else if shortcut_supported() {
                        eprintln!(
                            "[claude-hourglass] global shortcut {shortcut_str} registered"
                        );
                    } else {
                        eprintln!(
                            "[claude-hourglass] global shortcut {shortcut_str} registered but won't fire on Wayland — bind via DE"
                        );
                    }
                }
                Err(e) => {
                    eprintln!("[claude-hourglass] {e}");
                }
            }

            let toggle_item =
                MenuItem::with_id(handle, "toggle", "Show / Hide", true, None::<&str>)?;
            let refresh_item =
                MenuItem::with_id(handle, "refresh", "Refresh", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(handle, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(handle, &[&toggle_item, &refresh_item, &quit_item])?;

            let tray_icon = tauri::include_image!("icons/source/tray-icon.png");

            TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .tooltip("Claude Hourglass")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle" => toggle_popup(app),
                    "refresh" => {
                        let _ = app.emit("refresh-usage", ());
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_popup(tray.app_handle());
                    }
                })
                .build(handle)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_usage,
            get_config,
            set_session_key,
            set_position,
            set_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
