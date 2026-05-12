<div align="center">

# Claude Hourglass

A tray app with a popup window and global shortcut for live Claude session usage.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue?style=for-the-badge)](#license)
[![Built with Tauri 2](https://img.shields.io/badge/built%20with-Tauri%202-d97757?style=for-the-badge)](https://tauri.app)

<img src="docs/tray.png" width="480" alt="Orange sparkle tray icon in the GNOME panel" />

<br />

<img src="docs/screenshot.png" width="520" alt="Claude Hourglass popup showing 19% session used and 4h 42m until reset" />

</div>

---

## What it does

Lives in your system tray. Click the icon — or hit your bound keyboard
shortcut — and a small popup window shows the same numbers
`claude.ai/settings/usage` shows: current 5-hour session percent and
time until reset. Auto-refreshes every minute while open.

The popup's screen position (4 corners + center) and the global
shortcut combo are configurable via the gear icon in the popup
itself.

Sourced directly from claude.ai's own internal usage endpoint, so the
percent matches the dashboard exactly.

## Install

Download the latest artifact from
[**Releases**](https://github.com/AbdullahAlattar/claude-hourglass/releases),
then:

```sh
# Fedora / RHEL
sudo dnf install ./claude-hourglass-*.x86_64.rpm

# Ubuntu / Debian
sudo apt install ./claude-hourglass_*_amd64.deb
```

**macOS:** open the `.dmg`, drag **Claude Hourglass** to `/Applications`. The first launch is blocked by Gatekeeper because the app isn't signed with an Apple Developer certificate.

- **macOS 14 (Sonoma) or earlier:** right-click the app in Finder → **Open** → click **Open** in the dialog.
- **macOS 15 (Sequoia) or later:** Apple removed the Control-click bypass. Open **System Settings → Privacy & Security**, scroll to the bottom, find the message about Claude Hourglass being blocked, and click **Open Anyway** (admin auth required).

Subsequent launches work normally.

**Windows:** download `claude-hourglass_*_x64-setup.exe` (NSIS, **recommended** — installs per-user under `%LOCALAPPDATA%`, no admin prompt) or the `.msi` (Windows Installer, system-wide, triggers UAC). Double-click to install. On first launch Microsoft Defender SmartScreen will show **"Windows protected your PC"** because the binary isn't signed with a code-signing certificate — click **More info → Run anyway**. Subsequent launches open directly. The app's tray icon shows in the notification area (click the `^` chevron to expand if Windows hid it).

> **GNOME users:** the tray icon needs the
> [AppIndicator and KStatusNotifierItem Support](https://extensions.gnome.org/extension/615/appindicator-support/)
> extension.

## First-run setup

The popup launches with **Not connected**. Click the gear icon and paste
your Claude session cookie:

1. Open <https://claude.ai> in your browser, log in.
2. DevTools (`F12`) → **Application** tab → **Cookies** → `https://claude.ai`.
3. Click `sessionKey`. Copy the **Value** (starts with `sk-ant-sid01-…`).
4. Paste into the popup. Save.

The cookie is stored at `~/.config/claude-hourglass/config.json` on
Linux/macOS (mode `0600`, atomic write) or
`%APPDATA%\claude-hourglass\config.json` on Windows. It never leaves
your machine except to claude.ai itself. Sign out of claude.ai or
click **Disconnect** to revoke.

## Keyboard shortcut

Bind a keyboard shortcut in your desktop environment to toggle the
popup — point it at the binary with `--toggle`:

```
/usr/bin/claude-hourglass --toggle
```

<details>
<summary><b>GNOME</b></summary>

`Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts → +`

| Field | Value |
|---|---|
| Name | Claude Hourglass |
| Command | `/usr/bin/claude-hourglass --toggle` |
| Shortcut | any free combo (e.g. <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>L</kbd>) |
</details>

<details>
<summary><b>KDE Plasma</b></summary>

`System Settings → Shortcuts → Add Application → Custom`

| Field | Value |
|---|---|
| Action | `/usr/bin/claude-hourglass --toggle` |
| Trigger | any free combo |
</details>

<details>
<summary><b>XFCE</b></summary>

`Settings → Keyboard → Application Shortcuts → Add`

| Field | Value |
|---|---|
| Command | `/usr/bin/claude-hourglass --toggle` |
| Shortcut | any free combo |
</details>

<details>
<summary><b>macOS / Windows / Linux X11</b></summary>

The in-app global shortcut works natively — no DE binding needed.
Default: <kbd>Control</kbd>+<kbd>Option</kbd>+<kbd>L</kbd> on macOS,
<kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>L</kbd> on Windows / X11 Linux.

To change it: open the popup → click the gear icon → click the
**global shortcut** field → press your desired combo. The new
shortcut is saved to `~/.config/claude-hourglass/config.json` and
re-registered live. (On Wayland the field is disabled — bind via
your DE per the sections above instead.)
</details>

CLI flags supported: `--toggle`, `--show`, `--hide`.

## Auto-start at login

Save the following as `~/.config/autostart/claude-hourglass.desktop`.
This is the freedesktop XDG Autostart format — works on GNOME, KDE,
XFCE, Cinnamon, MATE, and any other freedesktop-compliant DE.

```ini
[Desktop Entry]
Type=Application
Name=Claude Hourglass
Exec=/usr/bin/claude-hourglass
```

**macOS:** `System Settings` → **General** → **Login Items** → click **+** → add **Claude Hourglass**.

**Windows:** press <kbd>Win</kbd>+<kbd>R</kbd>, type `shell:startup`, press Enter. In the folder that opens, right-click → **New** → **Shortcut**, and point it at the installed binary (NSIS default: `%LOCALAPPDATA%\Programs\Claude Hourglass\claude-hourglass.exe`; MSI default: `C:\Program Files\Claude Hourglass\claude-hourglass.exe`). The shortcut runs automatically next time you sign in.

## Build from source

```sh
# Fedora prerequisites
sudo dnf install \
  webkit2gtk4.1-devel librsvg2-devel libappindicator-gtk3-devel \
  openssl-devel gcc gcc-c++ make

# Ubuntu prerequisites
sudo apt install \
  libwebkit2gtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev \
  build-essential libssl-dev

# macOS prerequisites
xcode-select --install   # if not already installed (provides clang, git, etc.)
# Tauri uses native WKWebView on macOS — no extra system libs needed.

# Windows prerequisites
# Install "Build Tools for Visual Studio" (free) with the "Desktop development
# with C++" workload, or install Visual Studio Community. This provides MSVC,
# the Windows SDK, and the linker that the Rust msvc target needs.
# WebView2 ships with Windows 11 and is auto-installed by the Tauri installer
# on Windows 10 — no extra setup needed.

git clone https://github.com/AbdullahAlattar/claude-hourglass
cd claude-hourglass
npm install
npm run tauri build       # release artifacts in src-tauri/target/release/bundle/
npm run tauri dev         # development run
```

Requires Rust 1.77+ and Node 18+.

## Platform support

| Platform | Status |
|---|---|
| Linux (Fedora, Ubuntu, KDE, GNOME) | Tested |
| macOS (Apple silicon, Intel) | Tested |
| Windows 10 / 11 | Tested |

## Known limitations

- **Wayland window positioning**: GNOME's compositor decides where the
  popup appears (xdg-shell forbids apps from positioning their own
  toplevels). On X11/KDE the bottom-right anchor works.
- **Windows 10 cosmetic transparency**: a thin 1-pixel border may appear
  around the popup on Windows 10 due to an open upstream Tauri/WebView2
  issue ([tauri#13176](https://github.com/tauri-apps/tauri/issues/13176)).
  Rounded corners require Windows 11's DWM; on Windows 10 the panel
  renders with square corners. Functional behavior is unaffected.
- **Cookie expires** when you sign out of claude.ai. Re-paste from the
  gear icon when that happens.
- **Unofficial endpoint**: see Disclaimer below.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Bug reports, platform fixes,
and resilience to claude.ai endpoint changes are all welcome. Don't
expect feature creep — the popup is intentionally single-glance.

## Disclaimer

> This is an unofficial third-party tool. **Not affiliated with,
> endorsed by, or supported by Anthropic.** "Claude" is a trademark of
> Anthropic.
>
> Claude Hourglass reads from a non-public Claude endpoint
> (`/api/organizations/{org_id}/usage`) using your session cookie — the
> same one your browser uses. Anthropic could change or remove this
> endpoint without notice; the app would break until updated. Use at
> your own risk.

## License

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual-licensed as above, without any additional terms
or conditions.
