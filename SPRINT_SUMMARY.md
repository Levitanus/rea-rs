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

## Follow-up considerations: native containers and explicit event propagation

The next GUI layer should preserve the distinction between the native HWND hierarchy and
application-level event forwarding:

```text
child HWND
  -> immediate native parent/container callback
    -> local handling or aggregation
      -> optional explicit forwarding to a parent container or window
```

The current implementation has the callback contracts and registry foundation in
`rea-rs/src/swell_gui.rs`, including stable `ContainerId` values, widget/container callback
registration, direct control-to-container relationships, and an initial `WM_COMMAND` routing
path. This is not yet a complete native implementation: callbacks are not automatically invoked
by a custom container HWND procedure, `ForwardToParent` traversal is not operational, and
`WM_NOTIFY`, `WM_HSCROLL`, and `WM_VSCROLL` still need to use the same dispatch model.

The likely GroupBox design is a callback-capable event/layout container with an optional visual
GroupBox child, rather than treating the standard visual GroupBox control itself as the complete
container abstraction. A container should be able to aggregate direct-child state and forward a
higher-level event only when its callback explicitly requests it; logical widget-tree bubbling
should not be introduced implicitly.

ScrollView should be implemented as real stacked native HWNDs:

```text
ScrollView
├── Viewport
│   └── Content
│       └── child HWNDs
└── scrollbars
```

Scrolling should move or resize the Content surface while keeping child HWND ownership stable,
avoiding unnecessary child recreation or reparenting. Follow-up work should include resize/reflow
integration, lifecycle and rebinding tests, nested-container propagation tests, and manual
cross-platform validation.

## Results of the callback-registry work

- Added `ContainerId` as a stable container identity derived from `ControlId`.
- Added `NativeContainer` for container-oriented access to native controls and HWND rebinding.
- Added `EventResponse` and `ContainerResponse` to make handling and forwarding decisions explicit.
- Added `WidgetEventCallback` and `ContainerEventCallback` registration contracts.
- Added `EventRegistry` storage for widget callbacks, container callbacks, direct child ownership,
  and nested container relationships.
- Added `ReaperWindow` registration methods:
  - `on_widget_event(...)`
  - `on_container_event(...)`
  - `set_control_container(...)`
  - `set_container_parent(...)`
- Added initial `WM_COMMAND` routing through the widget callback and direct-container callback
  layers before optional top-level forwarding.
- Exported the new container and callback types from `rea-rs/src/lib.rs`.
- Added focused tests covering child-event/container identity preservation, stable native-container
  identity, and direct native-parentage tracking.
- Focused GUI tests passed: 3 tests passed, 0 failed.
- Affected-crate compilation passed for `rea-rs-low`, `rea-rs`, and
  `reaper-test-extension-plugin`.

This work establishes the API and registry foundation, but the callback path is not yet connected
to a custom native container window procedure. Native container HWND creation, complete forwarding,
additional message types, and stacked ScrollView construction remain follow-up work.
