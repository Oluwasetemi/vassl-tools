# Changelog

All notable changes to VASSL are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0-beta.8] - 2026-06-16

### Added
- Settings → Database: "Load Database" — opens a file picker to select a `.sqlite` backup file; shows an inline confirmation before replacing the current database. The selected file is staged and the app restarts automatically to open it cleanly.
- Settings → Database: "Reset Database" — shows an inline two-step confirmation (Reset… → Confirm Reset) before permanently deleting all data. The app restarts with an empty database and the first-run setup screen.

---

## [0.1.0-beta.7] - 2026-06-16

### Fixed
- "Download" button now stays visible while the update is downloading, shown at 50% opacity with the label "Downloading…" — matches the "Checking…" pattern introduced in beta.4.

---

## [0.1.0-beta.6] - 2026-06-15

### Fixed
- Database schema migration: products, suppliers, and projects tables were missing columns (`end_of_life`, `model_number`, `part_number`, `duty_percent`, `replacement`, `address`, `date_started`, and others) on databases originally created by an alpha build. All three stores now fail to load at startup on affected databases. Root cause: the migration system silently skips changed `CREATE TABLE` steps (`should_allow_migration_change = true`), so columns added to those initial steps were never applied to existing databases. Fix: added new migration steps for each affected domain that recreate the table with the full current schema and copy existing data, so all columns are present after upgrade regardless of which alpha version created the database.

---

## [0.1.0-beta.5] - 2026-06-15

### Fixed
- MSI installer: `AllowSameVersionUpgrades="yes"` added so previous installs are always removed first. Pre-release builds (alpha.N, beta.N) all produced the same Windows FILEVERSION `0.1.0.0`, so the installer couldn't tell them apart and installed alongside the old version instead of replacing it.
- MSI installer: `build.rs` now encodes a monotonically increasing 4th version component (`alpha.N→1000+N`, `beta.N→2000+N`) into the Windows PE FILEVERSION, so future MSI upgrades work on numeric version comparison alone.

---

## [0.1.0-beta.4] - 2026-06-15

### Added
- Auto-updater tracing: every decision point (channel check, GitHub API call, version comparison, download, install) now emits a structured log line to the daily log file.

### Fixed
- "Check for updates" button stays visible while checking, shown at 50% opacity, instead of disappearing entirely.
- Product form (create mode): Initial Stock tab stop now follows Replacement, matching visual order. End of Life checkbox is keyboard-reachable via Tab/Space.
- Line Item form: Product dropdown added as first tab stop.
- Auto-updater: replaced dead `update_url()` / placeholder `releases.vassl.app` URLs with `supports_updates() -> bool`.

---

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
