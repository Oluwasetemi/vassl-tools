# Changelog

All notable changes to VASSL are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0-beta.3] - 2026-06-14

### Added
- Project `CLAUDE.md` with agent skill configuration (issue tracker, triage labels, domain docs)
- `docs/agents/` directory with agent skill reference docs:
  - `issue-tracker.md` — GitHub Issues conventions for agent operations
  - `triage-labels.md` — canonical triage label mapping
  - `domain.md` — domain doc consumption rules for engineering skills

### Changed
- Bumped version to `0.1.0-beta.3`

## [0.1.0-beta.2] - 2026-06-12

### Added
- macOS code signing and notarization support in CI

### Fixed
- Inventory: fall back to global low stock threshold for products with no per-product minimum
- CI: extend macOS build artifact expiry to 14 days for the beta channel
- CI: embed `vassl.icns` into `Contents/Resources` after `cargo-bundle`
- CI: use job-level env for signing gate; fix checkout ordering
- Release: DMG creation and bundle metadata

## [0.1.0-beta.1] - 2026-06-08

### Added
- Authentication and user management
- Quotation editing
- Currency-aware pricing
- Auto-focus UX improvements
