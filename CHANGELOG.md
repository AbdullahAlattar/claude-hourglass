# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.7] - 2026-05-16

### Fixed

- **Silent popup hang on Linux with NVIDIA proprietary driver under
  Wayland.** Clicking the tray icon did nothing — no popup appeared,
  no error logged, no crash dialog. `webkit2gtk` 2.42+ defaults to a
  DMA-BUF–based renderer that shares GPU buffers zero-copy with the
  Wayland compositor, but the buffer-modifier handshake with the
  NVIDIA proprietary driver hangs indefinitely: the driver advertises
  DMA-BUF support but never completes the format negotiation Mutter
  expects. WebKit blocks waiting for a buffer that never arrives →
  popup has no backing surface → nothing renders. The tray icon still
  works because it goes through `libayatana-appindicator` (GTK only,
  no WebKit), so the failure mode is the confusing "tray click does
  nothing." Reproducible on Fedora 43 + GNOME 47 Wayland + `nvidia`
  555.x + `webkit2gtk-4.1` 2.52.x with an RTX 3080; same RPM works
  fine on the same OS with Intel/AMD Mesa. Set
  `WEBKIT_DISABLE_DMABUF_RENDERER=1` at process start in
  `src-tauri/src/main.rs` (Linux only, before Tauri initialises),
  falling back to shared-memory rendering. Slower than DMA-BUF on
  paper, invisible for a tray popup that paints a handful of numbers
  every 60s. macOS and Windows builds unaffected — they use WKWebView
  and WebView2 respectively, not webkit2gtk. Users on a working
  NVIDIA + Mesa NVK or future fixed driver/WebKit combo can opt back
  into DMA-BUF with `WEBKIT_DISABLE_DMABUF_RENDERER=0 claude-hourglass`.
  **NVIDIA + Wayland users on v0.1.6 or earlier should upgrade.**
  Mesa users on any version: upgrading is optional, behaviour is
  unchanged.

## [0.1.6] - 2026-05-12

### Security

- **Bumped `tauri` 2.11.0 → 2.11.1** to patch
  [CVE-2026-42184](https://github.com/advisories/GHSA-7gmj-67g7-phm9)
  (origin confusion in `is_local_url()` on Windows and Android). Tauri's
  `is_local_url()` only split the host on the first `.`, so a remote
  page at e.g. `http://app.evil.com/` was classified as a local
  `Origin::Local` and could invoke locally-scoped IPC commands.
  Windows-only impact; Linux and macOS were unaffected by this CVE.
  Our shipped surface for the attack was effectively zero — the app
  never opens external URLs in its WebView, has no deep-link handlers,
  no custom URI scheme registered beyond Tauri's default, and the
  popup loads only the bundled frontend bundle. But the vulnerable
  binary needed patching regardless. **v0.1.5 Windows installs should
  upgrade to v0.1.6.** Linux/macOS v0.1.5 binaries are not at risk and
  upgrading is optional.

### Known dependency advisory (not patched)

- **`glib` 0.18.5** triggers [GHSA-wrw7-89jp-8q8g](https://github.com/advisories/GHSA-wrw7-89jp-8q8g)
  (`VariantStrIter::impl_get` unsoundness, fixed in glib 0.20). This is
  a transitive dependency via Tauri's gtk-rs 0.18.x ecosystem and
  cannot be bumped without Tauri itself upgrading. The bug manifests
  as a potential NULL-pointer crash when enumerating GVariant strings
  — a Linux GTK code path our app doesn't exercise (we don't iterate
  GVariant string arrays). Reassessed when Tauri ships a release on
  gtk-rs 0.20.

## [0.1.5] - 2026-05-12

### Added

- **Windows support.** The release workflow now builds `.msi` (Windows
  Installer, system-wide via UAC) and `.exe` (NSIS, per-user, no admin)
  artifacts on `windows-latest` runners alongside the existing Linux
  `.rpm`/`.deb` and macOS `.dmg`. WebView2 is pulled in by the installer
  on Win10 if missing (preinstalled on Win11). Tray icon, popup, and
  in-app global shortcut (`Ctrl+Alt+L`) all work natively — no DE
  binding workaround needed, since Windows uses `RegisterHotKey` (same
  way macOS uses `NSEvent`).
- **Per-OS CI coverage.** Added a `cargo check` job on `windows-latest`
  in `ci.yml` so Windows-specific compile errors in
  `#[cfg(windows)]`-gated code are caught on every PR instead of at
  release tag time. `clippy` and `fmt` remain Linux-only since their
  output is platform-agnostic for this codebase.

### Fixed

- **Config persistence was broken on Windows.** `config_dir()` only
  consulted `XDG_CONFIG_HOME` and `HOME`, both of which are unset on a
  vanilla Windows install (Windows uses `USERPROFILE`). The function
  returned `None`, `write_config` errored out, and the session cookie
  could never be saved — making the app effectively unusable on
  Windows. Added a `cfg(windows)` branch that uses `%APPDATA%` (the
  Roaming profile), placing config at
  `%APPDATA%\claude-hourglass\config.json`. macOS keeps its existing
  XDG-style path at `~/.config/claude-hourglass/` so installs from
  v0.1.3/v0.1.4 aren't silently logged out by a path relocation.

### Known limitations on Windows

- **Windows 10** may render a thin 1-pixel border around the popup
  ([tauri#13176](https://github.com/tauri-apps/tauri/issues/13176)) and
  always renders square corners — DWM rounded corners are Windows 11
  only. Functional behavior is unaffected on both versions.
- **SmartScreen** shows "Windows protected your PC" on first launch
  because the binary isn't code-signed. Click **More info → Run anyway**
  to proceed (one-time). There is no ad-hoc signing equivalent on
  Windows — only paid OV/EV certificates (or Microsoft Trusted Signing)
  remove the warning. Same posture as the unsigned macOS build.

## [0.1.4] - 2026-05-12

### Fixed

- **macOS arm64 build was rejected as "damaged"** on Apple Silicon. The
  binary inside the `.dmg` was completely unsigned (no `LC_CODE_SIGNATURE`
  load command, no `_CodeSignature` directory in the `.app` bundle); arm64
  macOS requires *at least* an ad-hoc signature, so AMFI refused to launch
  the app even with quarantine removed. Added
  `bundle.macOS.signingIdentity = "-"` to `tauri.conf.json` so Tauri's
  bundler applies an ad-hoc signature via `codesign` during build.
  Users now see the standard "unidentified developer" Gatekeeper warning
  (with the documented right-click → Open or Settings → Privacy & Security
  → Open Anyway bypass) instead of the "damaged" wall.

### Workaround for v0.1.3 installs

If you already installed v0.1.3 and hit "damaged and can't be opened":

```sh
sudo xattr -dr com.apple.quarantine "/Applications/Claude Hourglass.app"
sudo codesign --force --deep --sign - "/Applications/Claude Hourglass.app"
```

Then double-click. Or just upgrade to v0.1.4.

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

[Unreleased]: https://github.com/AbdullahAlattar/claude-hourglass/compare/v0.1.7...HEAD
[0.1.7]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.7
[0.1.6]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.6
[0.1.5]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.5
[0.1.4]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.4
[0.1.3]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.3
[0.1.2]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.2
[0.1.1]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.1
[0.1.0]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.0
