# rea-rs

A high-level, fallible Rust API for REAPER extension plug-ins. `rea-rs` builds on the workspace's low-level bindings and offers typed project, track, item, take, MIDI, envelope, source, and SWELL GUI APIs.

This crate targets Rust 1.82 or newer and REAPER extension plug-in development. Host-dependent operations must run in REAPER and generally on its UI/main thread.

See the [workspace README](../README.md) for examples, and the [1.0.0 API audit](../RELEASE_1.0.0_API_AUDIT.md) for release status, breaking changes, and migration notes.
