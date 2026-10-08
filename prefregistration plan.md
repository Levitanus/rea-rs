## Plan: Use-case-driven extension GUI architecture

### Decisions from latest discussion
- Toolbar overflow order: wrapping first, scrolling second, clipping only when explicitly selected.
- Editor profile: investigate and adopt an egui-inspired panel model, without initially embedding egui itself.
- `Fill` and `FillPortion` are first-class sizing policies for widgets and containers.
- REAPER Preferences integration is the most important investigation and should be treated as a dedicated host-owned page API, not an ordinary `ReaperWindow`.

### Recommended architecture
Build a pure retained geometry/layout layer and keep native SWELL controls plus current `ControlId` event routing authoritative. The layout tree should produce rectangles and overflow metadata; the window layer applies rectangles to HWND controls. Logical containers do not automatically become native HWND parents.

Profiles:
1. `Toolbar`: one-dimensional wrapping flow, groups, `Fill`/`FillPortion`, explicit overflow policy.
2. `PreferencesPage`: dedicated REAPER `prefpage` registration and host-parented page lifecycle.
3. `Editor`: egui-like edge panels and central panel, nested rows/columns, optional scroll and custom canvas later.

### Toolbar model
Use `OverflowPolicy::{Wrap, Scroll, Clip}` with `Wrap` as default.
- Measure preferred/minimum sizes first.
- Form lines/columns using preferred main-axis sizes plus spacing.
- Allocate `Fill`/`FillPortion` only within each line/column.
- Groups are atomic parent-flow items by default and can wrap internally.
- `Scroll` preserves content extent and requires a later viewport/content native subsystem.
- `Clip` is explicit and reports clipped items/overflow; it must never be implicit.
- Docked windows are treated as host-controlled viewports; do not force REAPER to satisfy layout minimums.
- Use one axis-parameterized flow algorithm for horizontal and vertical toolbars.

Sizing vocabulary:
- `Fixed`
- `Preferred`
- `Fill`
- `FillPortion(u32)`
- min/max constraints

### egui-inspired editor model
Use the conceptual pattern:
- top/bottom edge panels consume height;
- left/right side panels consume width;
- central panel receives remaining space;
- each region can contain nested rows/columns and native controls.

Initial nodes/concepts:
- `Region`
- `TopPanel` / `BottomPanel`
- `SidePanel`
- `CentralPanel`
- `Row` / `Column`
- `Group`
- `Canvas` (future paint surface)
- `ScrollArea` (separate native subsystem)

Do not embed egui: egui expects ownership of painting, input, focus, hit-testing, and widget state, while this codebase currently depends on native HWND controls and `WindowHandler` routing. Adopt the geometry/allocation idea only.

### Preferences-page investigation findings
The raw REAPER ABI already exists:
- `prefs_page_register_t` in `/home/levitanus/gits/rea-rs/low/lib/reaper/reaper_plugin.h` and generated `/home/levitanus/gits/rea-rs/low/src/bindings.rs`.
- Registration is via `plugin_register("prefpage", &registration)`; unregister convention is likely `plugin_register("-prefpage", &registration)`.
- Low-level `plugin_register` is already wrapped in `low/src/reaper.rs`; high-level registration patterns exist in `/home/levitanus/gits/rea-rs/rea-rs/src/reaper.rs`.
- `Reaper::view_prefs` only opens Preferences; it does not register a page.
- `prefs_page_register_t` is not currently re-exported through the intended raw facade.

ABI/lifecycle constraints:
- `create(HWND parent) -> HWND` is an `extern "C"` callback; it must be panic-barriered and return null on failure.
- C strings and the registration record must remain at stable addresses for the entire registration lifetime.
- REAPER owns the preferences parent HWND. The extension returns a child/page HWND and must not destroy the parent.
- The current `ReaperWindow` ownership/docking model is not an appropriate abstraction for this host-owned page.
- There is no explicit destroy/resize callback in the registration struct; page creation/recreation/destruction behavior must be verified experimentally.
- `treeitem` and `hwndCache` should be treated as host-owned opaque fields.
- `Reaper` shutdown currently does not unregister pref pages; explicit unregister must be added before registration state/strings/callback state are dropped.
- Unknowns: repeated `create`, callback timing, page destruction, resize delivery, exact unregister payload/behavior, Preferences open during unload, and main-thread requirements.

### Proposed Preferences API
Create a dedicated `PreferencesPage`/`PreferencesPageRegistration` abstraction with explicit host ownership:
- `PreferencesPageSpec`: stable ID, display name, optional parent page ID/string, children flag, creation handler.
- `PreferencesPageContext`: borrowed host parent HWND.
- `PreferencesPageWindow`: page HWND and direct-child control registry; dropping it must not destroy the host parent.
- Stable boxed registration record plus owned `CString` fields and callback state.
- Callback dispatch through a stable registry keyed by page ID or registration address, wrapped with the existing panic firewall.
- Explicit main-thread register/unregister and idempotent shutdown cleanup.
- Reuse existing control creation/event decoding where possible, but add host-page-specific message/lifecycle routing rather than pretending the page is a normal top-level window.

### Investigation/prototype before implementation
Build a minimal native/runtime probe before finalizing the public API:
1. Register one page with a stable ID/display name.
2. Log every `create` call and supplied parent HWND.
3. Create one native control and handle resize if delivered.
4. Open/switch/reopen Preferences repeatedly.
5. Observe page HWND destruction and whether `hwndCache` changes.
6. Test unregister while Preferences is closed and open.
7. Confirm exact `-prefpage` behavior and shutdown ordering.
8. Verify Linux/SWELL and Windows behavior separately where available.

### Relevant files
- `/home/levitanus/gits/rea-rs/rea-rs/src/swell_gui.rs` — pure layout integration, existing control/event routing, future host-page message routing.
- `/home/levitanus/gits/rea-rs/rea-rs/src/reaper.rs` — registration ownership, plugin registration, shutdown cleanup.
- `/home/levitanus/gits/rea-rs/rea-rs/src/simple_functions.rs` — `view_prefs` distinction.
- `/home/levitanus/gits/rea-rs/low/src/bindings.rs` — generated pref-page ABI; do not edit generated code directly.
- `/home/levitanus/gits/rea-rs/low/src/raw.rs` — re-export surface to extend.
- `/home/levitanus/gits/rea-rs/low/lib/reaper/reaper_plugin.h` — authoritative ABI.
- `/home/levitanus/gits/rea-rs/test/src/swell_gui.rs` — toolbar/editor gallery validation.
- `/home/levitanus/gits/rea-rs/rea-rs/src/layout.rs` — proposed pure geometry module.

### Phases
1. Preferences runtime probe and ABI/lifecycle confirmation.
2. Pure geometry contracts and tests: `Rect`, `Size`, `Insets`, orientation, sizing policies, overflow metadata.
3. Toolbar wrapping allocator and logical groups; direct-child native integration.
4. Eg​​ui-inspired editor shell: edge panels, side panels, central fill, nested rows/columns.
5. Dedicated Preferences page API and page-specific control lifecycle.
6. Native ScrollArea for explicit toolbar/editor overflow.
7. Custom canvas paint contract.

### Verification
- Toolbar wrapping thresholds, vertical/horizontal symmetry, groups, `Fill`, `FillPortion`, min/max, deterministic rounding, and explicit overflow.
- Repeated resize/dock/redock preserves control IDs and event delivery.
- Preferences page appears, creates/recreates correctly, resizes, unregisters cleanly, and does not destroy host-owned HWNDs.
- Editor edge-panel allocation leaves the central region with the exact remaining rectangle.
- Scroll and painting are not considered complete until clipping, input/event routing, rebinding, and invalidation are tested.

### Scope boundaries
Do not implement a full generic widget framework, embed egui, add arbitrary grid/baseline/font measurement, or add ScrollView/custom painting before the core profiles and host-page lifecycle are validated.
