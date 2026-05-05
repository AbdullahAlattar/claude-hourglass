# Contributing to Claude Hourglass

Thanks for considering a contribution. This is a small focused tool —
the priorities are correctness, simplicity, and respect for the
Anthropic-style visual identity. PRs that move in a different aesthetic
direction or expand scope significantly are likely to be declined.

## Development setup

```sh
# Fedora prerequisites
sudo dnf install \
  webkit2gtk4.1-devel \
  librsvg2-devel \
  libappindicator-gtk3-devel \
  openssl-devel \
  gcc gcc-c++ make

# Ubuntu/Debian prerequisites
sudo apt install \
  libwebkit2gtk-4.1-dev \
  librsvg2-dev \
  libayatana-appindicator3-dev \
  build-essential \
  libssl-dev

# Install Rust 1.77+ and Node 18+, then:
git clone https://github.com/AbdullahAlattar/claude-hourglass.git
cd claude-hourglass
npm install
npm run tauri dev
```

See `README.md` for end-user setup (paste claude.ai cookie, bind a DE
shortcut, etc.).

## Before opening a PR

Run these locally — CI runs the same checks:

```sh
cd src-tauri
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo check
cd ..
npx tsc --noEmit
```

If `cargo fmt --check` fails, run `cargo fmt --all` to fix.

## What's in scope

- **Bug fixes** — anything that misrepresents your usage vs claude.ai's
  dashboard, or breaks the popup/tray on supported platforms.
- **Platform support** — macOS and Windows aren't actively tested; PRs
  that fix paper cuts there are welcome (open an issue first to discuss).
- **Small UX improvements** — better error states, autostart helper,
  libsecret/Keyring storage for the cookie.
- **Resilience to claude.ai endpoint changes** — defensive parsing,
  better error messages.

## What's out of scope

- Adding chart libraries or a "history" view. The popup is intentionally
  one-glance.
- Replacing the visual style. Anthropic-cream + Source Serif 4 is a
  deliberate choice.
- Hosted services, telemetry, analytics, accounts.
- Bundling official Anthropic API keys for cost tracking — the API key
  would belong to whoever runs the app.

## Filing issues

- **Bug:** include your distro, GNOME / KDE version, output of
  `npm run tauri dev` from the moment you triggered the bug, and what
  the popup's console (right-click → Inspect → Console) showed.
- **Feature:** describe the user-facing change first, implementation
  later. If it adds a dependency, justify it.

## Commit style

No formal convention. Imperative mood (`Fix`, `Add`, `Refactor`).
One topic per commit. Avoid "wip" / "fix typo" noise — squash before
opening the PR.

## License

By contributing, you agree your contribution is dual-licensed under
**MIT OR Apache-2.0**, the same terms as the project.
