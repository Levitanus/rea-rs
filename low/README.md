# rea-rs-low

Low-level Rust bindings and C++ glue for the REAPER extension API, used by the `rea-rs` workspace.

The crate vendors the Cockos WDL and REAPER SDK headers needed for normal builds. Ordinary builds do not fetch upstream sources. To explicitly refresh vendored snapshots and regenerate the bindings on Linux, run `cargo update-sdk` from the workspace root.

See the workspace [README](../README.md) for project background, and the [1.0.0 API audit](../RELEASE_1.0.0_API_AUDIT.md) for release status and migration details.
