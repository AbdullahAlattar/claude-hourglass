# Changelog

All notable changes to this project will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/AbdullahAlattar/claude-hourglass/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/AbdullahAlattar/claude-hourglass/releases/tag/v0.1.0
