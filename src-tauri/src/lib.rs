use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

const POPUP_LABEL: &str = "main";
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0";

// ── Config ───────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Default, Clone)]
struct AppConfig {
    /// Session cookie copied from claude.ai (sessionKey value).
    session_key: Option<String>,
    /// Cached organization UUID — auto-discovered on first call.
    org_id: Option<String>,
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
/// Moves `~/.config/claude-usage-tray/config.json` → `~/.config/claude-hourglass/config.json`
/// when the new path is empty and the legacy one exists.
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

    // Atomic write: create tmp file with 0600 from the start, then rename
    // over the destination. Avoids the TOCTOU window of write-then-chmod
    // where the cleartext file briefly has default umask permissions.
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

// ── claude.ai API ────────────────────────────────────────────────

fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT)
        // Don't follow 3xx — claude.ai redirects unauthenticated requests
        // to the login HTML page, which would then fail JSON parsing with
        // a misleading error. Treat redirects as auth failures explicitly.
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
                // Re-read before patching — if the user changed session_key
                // while we were discovering the org, our cached `cfg` is stale.
                // Only persist the new org_id when the key still matches what
                // we discovered against; otherwise the next call will rediscover.
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
fn get_config() -> AppConfig {
    let cfg = read_config();
    // Don't echo session_key back to the frontend except as a presence flag.
    AppConfig {
        session_key: cfg.session_key.as_ref().map(|_| "***".to_string()),
        org_id: cfg.org_id,
    }
}

#[tauri::command]
fn set_session_key(key: Option<String>) -> Result<(), String> {
    let mut cfg = read_config();
    cfg.session_key = key.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    cfg.org_id = None; // force rediscovery next call
    write_config(&cfg)
}

// ── Window helpers ───────────────────────────────────────────────

fn toggle_popup(app: &AppHandle) {
    let Some(window) = app.get_webview_window(POPUP_LABEL) else {
        return;
    };
    match window.is_visible() {
        Ok(true) => {
            let _ = window.hide();
        }
        _ => {
            let _ = position_bottom_right(&window);
            let _ = window.show();
            let _ = window.set_focus();
            let _ = app.emit("popup-shown", ());
        }
    }
}

fn position_bottom_right(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let size = monitor.size();
    let win_size = window.outer_size()?;
    let margin = (16.0 * scale) as i32;
    let x = size.width as i32 - win_size.width as i32 - margin;
    let y = size.height as i32 - win_size.height as i32 - margin;
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
                    let _ = position_bottom_right(&window);
                    let _ = window.show();
                    let _ = window.set_focus();
                    let _ = app.emit("popup-shown", ());
                }
            } else if args.iter().any(|a| a == "--hide") {
                if let Some(window) = app.get_webview_window(POPUP_LABEL) {
                    let _ = window.hide();
                }
            }
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed
                        && shortcut.matches(Modifiers::CONTROL | Modifiers::ALT, Code::KeyL)
                    {
                        toggle_popup(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let handle = app.handle();

            // In-app shortcut is best-effort: works on X11/Windows/macOS,
            // silently fails on Wayland. On Linux you should bind via your
            // DE's Custom Shortcuts to `claude-usage-tray --toggle`.
            let shortcut =
                Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyL);
            if let Err(e) = handle.global_shortcut().register(shortcut) {
                eprintln!("[claude-hourglass] in-app shortcut Ctrl+Alt+L failed: {e}");
            } else {
                eprintln!(
                    "[claude-hourglass] in-app shortcut Ctrl+Alt+L registered (Wayland: bind via DE)"
                );
            }

            let toggle_item =
                MenuItem::with_id(handle, "toggle", "Show / Hide", true, None::<&str>)?;
            let refresh_item =
                MenuItem::with_id(handle, "refresh", "Refresh", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(handle, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(handle, &[&toggle_item, &refresh_item, &quit_item])?;

            // Custom monochrome tray icon (designed to read at panel sizes).
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
            set_session_key
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
