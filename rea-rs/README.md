# rea-rs

`rea-rs` is a high-level Rust API for working with REAPER from an extension.
It builds on [reaper-rs](https://github.com/helgoboss/reaper-rs), and its
design was inspired by [ReaPy](https://github.com/Levitanus/reapy-boost). The
goal is to make common REAPER tasks feel natural in Rust, with ownership and
lifetimes to manage host objects, typed values, and fallible calls when a
native operation can fail.

- [Published API documentation](https://levitanus.github.io/rea-rs-doc/rea_rs/index.html)
- [Workspace README and examples](../README.md)
- [1.0.0 API audit and migration notes](../RELEASE_1.0.0_API_AUDIT.md)
- [SWELL GUI extension example](../test/src/swell_gui.rs)

## Finding your way around

Start with `Reaper`: it is the host entry point for extension initialization,
host services, and registering actions, timers, control surfaces, and
windows. From there, get the current `Project` and work with its `Track`s
and `Item`s. An `Item` gives you its active `Take`. For more specialized
tasks, there are APIs for MIDI, FX, envelopes, sources, audio accessors,
ExtState, and SWELL GUI.

Most host operations return `ReaperResult<T>`, and callbacks commonly use
`anyhow::Result`. Pass these errors along or handle them where it makes sense;
they let your extension respond when a native operation doesn't succeed.

REAPER objects can be removed while their Rust wrappers still exist, so a
wrapper's lifetime does not guarantee that its native pointer is still valid.
`WithReaperPtr` checks validity internally whenever an object's `get()`
accesses that pointer. Because checking has some cost, use
`WithReaperPtr::with_valid_ptr` around a batch of operations on the same
object when appropriate; it validates once and temporarily skips repeat
checks for the closure. The object must remain valid throughout that closure.

The [workspace README](../README.md) walks through the API with examples: it
renames the active take of a selected item and shows how to register an
Action, `Timer`, `ControlSurface`, and `WindowHandler`. For a full GUI
extension, have a look at the
[SWELL GUI test](../test/src/swell_gui.rs): it builds a dockable widget
gallery and a MIDI editor overlay.

If you would like to run your own host-based tests, the
[`rea-rs-test` crate](../rea-rs-test/README.md) explains how to set up a
non-published `cdylib` test extension, register test steps behind a REAPER
Action, and run those steps on different REAPER versions. The workspace
README also describes the Sublime Text and VS Code build systems/tasks for
building the test plug-in and starting the integration test.

## Compatibility and API status

Version 1.0.0 is the canonical, breaking API, so names from before 1.0 are
not kept as compatibility aliases. The API is ready to use, though coverage
isn't uniform across all of REAPER and there is still audit and release work
to finish. The [1.0.0 API audit](../RELEASE_1.0.0_API_AUDIT.md) has the
details, including migration notes and remaining publication checks.

The manifest declares Rust 1.82 as its minimum version. This crate is aimed
at REAPER extension plug-ins, so host-dependent work generally belongs on
REAPER's UI/main thread. Remember to initialize the global `Reaper` instance
from the plug-in context before using its global accessors.
