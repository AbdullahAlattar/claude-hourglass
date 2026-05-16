// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Workaround for NVIDIA proprietary driver + Wayland + webkit2gtk: DMA-BUF
    // buffer-modifier negotiation hangs, leaving the popup window with no
    // backing surface (tray icon shows, click does nothing). Falling back to
    // SHM rendering is slower but works on every Linux GPU/compositor combo.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    claude_hourglass_lib::run()
}
