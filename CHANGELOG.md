# Changelog

## Unreleased

### Added

- `spek log` now shows commit descriptions, wrapping long lines with consistent indentation.

### Fixed

- Allow installation with Rust 1.98.0 by lowering the declared minimum and testing releases with that version.

## [1.0.0] - 2026-11-01

### Added

- Compact repository-wide Git status and diff statistics via `spek` / `spek diff`.
- Compact commit history via `spek log`.
- Per-checkout local diff exclusions with `exclude` and `include`.
- Handling for untracked, binary, renamed, unusual-path, and subdirectory cases.
