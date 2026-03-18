# Changelog

## Unreleased

### Added

* Added `update` feature flag to `rea-rs-low` build, allowing one-command refresh of vendored REAPER SDK headers and Cockos WDL sources (plus code generation).

### Changed

* WDL source refresh now automatically normalizes nested `WDL/WDL` checkouts into `WDL` and removes clone metadata files from vendored sources.

### Deprecated

### Fixed

### Removed

### Security

## 0.1.0 - 2020-05-07

* Initial release