# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.3] - 2026-05-07

### Added

- **macOS support.** Release workflow builds `.dmg` artifacts on
  macos-latest runners alongside Linux `.deb`/`.rpm`. The in-app
  global shortcut works natively on macOS (no DE-binding workaround
  needed) — default <kbd>Control</kbd>+<kbd>Option</kbd>+<kbd>L</kbd>.
- **Popup position setting.** 3×3 picker in the settings overlay lets
  you anchor the popup to any of the four corners or screen center.
  Default: bottom-right. Persists in `config.json`, applies
  immediately while the popup is visible.
- **Customisable global shortcut.** Click the shortcut field in
  settings → press a combo → it's saved and re-registered. On
  Linux/Wayland the field is disabled with a note explaining that
  the shortcut must be bound via the desktop environment's keyboard
  settings instead.
- **Fade-out animation on hide.** The popup now fades out (220 ms)
  when dismissed via tray click, shortcut, close button, or `Esc`
  — symmetric with the fade-in. All five hide paths route through a
  single frontend-driven flow so no path skips the animation.

### Changed

- **Fade-in animation** is now opacity-only at 220 ms `ease-in-out`
  (was a 280 ms spring slide-in). Smoother, no overshoot, no
  vibration on settle.
- **Wayland detection** now also checks `XDG_SESSION_TYPE` (with a
  `starts_with("wayland")` match for variants) in addition to
  `WAYLAND_DISPLAY`, catching nested-compositor and SSH-forwarded
  sessions where one var is set without the other.
- **macOS Gatekeeper docs** in README and release notes now split
  Sonoma/earlier (right-click → Open) from Sequoia/later (System
  Settings → Privacy & Security → Open Anyway), since macOS 15
  removed the right-click bypass.

### Fixed

- **Opaque white frame around the rounded panel on macOS.** Tauri's
  `transparent: true` only fully propagates to the WKWebView when
  the `macos-private-api` Cargo feature is compiled in. Enabled via
  the `macos-private-api` feature on the `tauri` crate plus
  `macOSPrivateApi: true` in `tauri.conf.json`. The 6 px panel inset
  now reveals the desktop as intended.
- **White-flash on first show under macOS WKWebView.** WebKit was
  painting unstyled HTML before the external stylesheet loaded.
  Inlined a critical `<style>` block in `<head>` so the transparent
  body background is applied immediately.
- **Animation invisible after the slide-in switch.** Removing
  `.is-shown` triggered a 1 → 0 opacity transition; the class was
  re-added two `requestAnimationFrame`s later (~32 ms), but
  opacity had only dropped to ~0.91 by then, so the next transition
  reversed across a ~9 % delta — imperceptible. `playShowAnimation`
  now temporarily disables the transition to snap opacity to 0
  before re-enabling it for a clean 0 → 1 fade.
- **Setup form rendered behind the empty/error overlay.** Direct
  `setupOverlay.hidden = false` left the underlying overlay visible
  on top (same z-index, later in DOM order). `openSetup()` now
  routes through `showOverlay("setup")` so all overlay flags update
  atomically.
- **Escape during shortcut capture closed the setup overlay.**
  `stopPropagation` doesn't block sibling listeners on the same
  node — switched to `stopImmediatePropagation` so Escape only
  cancels capture.
- **Outside-click leaves shortcut capture armed.** Clicking the
  shortcut button then back into the cookie input would let the
  next keystroke be hijacked as the new shortcut. Added a
  `pointerdown` listener (capture phase) that cancels capture on
  any click outside the shortcut field.
- **Tab and unrecognised keys swallowed in shortcut capture.**
  `preventDefault` was called before checking whether the key was
  usable; Tab couldn't escape capture mode. Now only consumed keys
  call `preventDefault`.
- **`set_shortcut` left the user with no shortcut on register
  failure.** The new shortcut is parsed and the old one is captured
  before any unregister; if `register(new)` fails, the old shortcut
  is re-registered before returning the error.
- **`parse_shortcut` accepted bare keys.** A user-edited
  `config.json` could specify `"L"` (no modifiers), which would
  register a system-wide single-letter grab. Now requires at least
  one modifier.
- **Position-picker UI lied on `set_position` failure.** The cell
  highlight applied optimistically before the Rust call resolved.
  Now applies only after the invoke succeeds.
- **`openSetup()` had no error handling for `get_config`.** Wrapped
  in try/catch; failures surface in the error overlay instead of
  silently breaking.
- **`dataset.position` cast was unchecked.** A future markup typo
  would round-trip a bogus value to Rust; now validated against an
  allow-list before invoking.

## [0.1.2] - 2026-05-05

### Fixed

- "Not connected" panel was unresponsive: clicking it ran the handler
  but the setup form rendered behind the empty overlay (same z-index,
  later in DOM order). Routed `openSetup()` and `closeSetup()` through
  `showOverlay()` so overlay state is set atomically.

## [0.1.1] - 2026-05-05

### Fixed

- Popup didn't accept clicks under native Wayland — switched the
  window to focus-on-show so GNOME's compositor delivers pointer
  events to the WebKit process.

### Changed

- "Not connected" panel: clicking anywhere on it now opens the setup
  form (you no longer have to find the gear icon).
- Top-right controls (settings / refresh / close) sit above all
  overlays so they remain reachable while an overlay is on.

## [0.1.0] - 2026-05-05

### Added

- Tray indicator + popup window showing live Claude.ai 5-hour-block
  usage and reset countdown, sourced from
  `claude.ai/api/organizations/{org_id}/usage`.
- One-tap setup flow for pasting the `sessionKey` cookie.
- 60-second auto-refresh while the popup is open.
- Single-instance plugin + `--toggle` / `--show` / `--hide` CLI flags
  so DE custom keybindings can drive the popup.
- Anthropic-styled visual identity: cream paper background, Source
  Serif 4 + Inter typography, single coral accent.
- Self-hosted fonts (no Google CDN dependency at runtime).
- Atomic config writes at `0600` to keep the cookie file private.

[Unreleased]: https://github.com/AbdullahAlattar/claude-hourglass/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.3
[0.1.2]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.2
[0.1.1]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.1
[0.1.0]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.0
