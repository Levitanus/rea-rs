# SWELL Windowing Sprint Summary

Date: 2026-10-02

## Goal

Implement a high-level, cross-platform SWELL windowing API for `rea-rs`, focused on window creation, ownership, docking/floating, destruction, resize handling, timers, and lifecycle callbacks. Widgets, painting systems, and a complete input framework remain out of scope.

## Implemented

### Low-level window creation

- Added hand-written SWELL/Win32 window creation in `low/src/swell_impl.rs`.
- Unix uses template-less `SWELL_CreateDialog` creation.
- Windows uses `RegisterClassExW` and `CreateWindowExW`.
- Added support for:
  - title and initial size
  - resizable/fixed window styles
  - minimize and close decoration options
  - top-level ownership by REAPER's main window
  - fallback Windows window procedure
  - Windows `GetWindowLong`, `SetWindowLong`, and `DefWindowProc` facades
- Added standard SWELL/REAPER background painting support.

### High-level API

Added `rea-rs/src/swell_gui.rs` containing:

- `WindowSpec`
- `ReaperWindow`
- `WindowHandler`
- `WindowCommand`
- `CommandNotification`
- `DockPosition`
- `WindowId`

`ReaperWindow` supports:

- owned and borrowed HWND wrappers
- safe null/invalid-window error handling
- showing and hiding
- title access
- client/window rectangles
- position and resize operations
- enable/disable and focus
- region invalidation with `Option<&RECT>`
- synchronous `send_message`
- asynchronous `post_message`
- periodic timers
- docking, floating, docker refresh, and docker-position queries
- explicit destruction and safe `Drop` behavior

### Event routing

- Added a SWELL window procedure/trampoline.
- Added callbacks for:
  - open
  - close/veto
  - destroy
  - resize
  - activate
  - timer
  - typed `WM_COMMAND`
- `WM_COMMAND` is decoded into menu commands and control notifications.
- Docker-close `IDCANCEL` notifications are handled as close requests.
- Unknown messages are forwarded to the platform default procedure.

### Reaper lifecycle integration

`Reaper` now:

- creates owned windows parented to REAPER's main HWND
- stores `WindowHandler` objects in a registry
- registers/unregisters handlers
- routes callbacks through the registry
- destroys owned windows during plugin unload
- removes docker registrations before destruction

### Docking and floating behavior

The implementation handles SWELL's distinction between logical windows and native surfaces:

- explicitly restores top-level ownership after undocking
- restores `WS_CAPTION` and `WS_THICKFRAME`
- applies `SWP_FRAMECHANGED`
- forces native surface recreation after parent transitions
- restores the saved floating rectangle after recreation
- preserves floating geometry separately from docker geometry
- tracks logical docking state for owned windows
- supports fresh wrappers around HWNDs using `from_hwnd_with_dock_ident`

### Window placement persistence

- Reused the existing `ExtState<Reaper>` abstraction.
- Window placement is keyed by logical `WindowSpec::dock_ident`, not by HWND or SWELL `resid`.
- Placement is stored under the `rea-rs.window` section.
- Stored placement includes left, top, width, and height.
- Docker dimensions are not used as floating-window geometry.
- `ext_state.rs` was not modified.

### Test/demo infrastructure

Added `test/src/swell_gui.rs` with explicit REAPER actions:

- `test rea-rs windowing`
- `test rea-rs windowing: toggle dock/float`

The demo logs lifecycle, resize, command, and docking events and tests wrapping the main window.

The windowing demo is intentionally an action-driven manual test rather than an automatic integration-test step.

### Dependency cleanup

Removed obsolete egui-baseview integration and unused `baseview`/`raw-window-handle` dependencies. The old GUI module had already been removed from the branch.

## Validation

The following builds passed during the sprint:

- `cargo build -p rea-rs-low`
- `cargo build -p rea-rs`
- `cargo build -p reaper-test-extension-plugin`
- combined builds of all three crates
- `cargo test -p rea-rs --lib` with 15 tests passing

Existing workspace warnings remain, primarily unrelated unused dependencies and mutable-static compatibility warnings.

## Important design decisions

- `resid` is only a SWELL creation parameter and is not a logical window ID.
- `dock_ident` is the stable logical identity for placement and docking state.
- HWNDs are runtime handles and may be replaced or reparented during docking.
- `ExtState<Reaper>` is preferred over INI storage for session-scoped placement.
- Generated low-level files remain off-limits; changes belong in hand-written extension files.
- Error paths use `ReaperResult` and avoid panics in new high-level code wherever possible.

## Known limitations

- A bare top-level window without child widgets normally produces few or no `WM_COMMAND` events.
- Full widget, paint, mouse, keyboard, menu, and modal-dialog APIs are not included.
- Cross-platform manual testing remains important, especially for native SWELL backends and Windows.
