## Backup Plan: Native-first responsive layout system

### Status
This is a backup plan only. Do not implement until the next layout idea is discussed and approved.

### Assessment
The prior proposal is a useful target architecture but too ambitious and does not solve the fundamental problem by itself: deciding which object owns geometry, how content behaves when space is insufficient, and how native HWND parenting affects clipping, events, and docking.

Estimated quality:
- 8/10 as a long-term architecture
- 5/10 as a first implementation scope

The safest direction is to keep the public concepts understandable while minimizing the number of independent geometry algorithms.

### Goals
- Keep logical layout identity separate from native HWND identity.
- Make resize and docking recompute geometry from current client bounds.
- Support fixed and responsive control arrangements without manual coordinate arithmetic.
- Allow content to exceed a viewport without fighting REAPER docking.
- Preserve reliable control notifications and HWND rebinding.
- Keep native SWELL/Win32 behavior authoritative.

### Proposed layout modes

#### `Fixed`
- Child inherits only the translated parent position.
- Authored width and height remain unchanged.
- No stretching, shrinking, wrapping, or redistribution.

#### `Rows`
- One horizontal row only.
- Never wraps.
- Includes `justify_x`, `justify_y`, `padding`, `spacing`, and constraints.
- Allocates children along the horizontal axis.
- Children may stretch or shrink according to sizing policy and min/max constraints.

#### `Columns`
- One vertical column only.
- Never wraps.
- Includes `justify_x`, `justify_y`, `padding`, `spacing`, and constraints.
- Allocates children along the vertical axis.
- Children may stretch or shrink according to sizing policy and min/max constraints.

#### `Stack`
- Explicit flow/wrapping layout.
- Horizontal flow creates additional rows when width is exhausted.
- Vertical flow creates additional columns when height is exhausted.
- Uses the same child measurement/allocation rules as Rows and Columns, but with wrapping enabled.

#### `Box`
- Responsive boundary rather than a second independent flow algorithm.
- Applies available bounds, padding, constraints, and justification to its child layout.
- May contain Fixed, Rows, Columns, Stack, or nested Box nodes.
- If wrapping is required, it delegates to Stack rather than implementing separate wrapping semantics.

#### `ScrollView`
- A viewport/content boundary, not only a rectangle-calculation mode.
- Content may exceed the viewport on enabled axes.
- Should preserve content layout coordinates separately from viewport coordinates.

### Internal implementation simplification
Use a small number of geometry algorithms:

1. Fixed placement.
2. Single-axis allocation for Rows and Columns.
3. Wrapping allocation for Stack.
4. Box boundary/constraint application that delegates child arrangement.
5. Scroll coordinate translation around a layout subtree.

Initial sizing vocabulary:
- `Fixed`
- `Fill`
- `FillPortion`
- `Shrink`

Initial constraints:
- minimum width/height
- maximum width/height
- padding and spacing

Defer intrinsic font measurement, baseline alignment, grid layout, and complex cross-axis policies.

### Window minimum-size policy
Keep three concepts separate:

1. Layout minimum: minimum extent preferred by content.
2. Floating minimum: minimum enforced through `WM_GETMINMAXINFO`.
3. Docked viewport: actual space supplied by REAPER.

Floating windows may use configured or layout-derived minimum dimensions. Docked windows must be allowed to become smaller than the layout minimum. When that happens, the layout must wrap, clip, or scroll rather than repeatedly resizing the host.

### ScrollView native architecture
A complete arbitrary-child ScrollView likely needs a composite native structure:

- viewport child HWND fixed to the parent layout rectangle
- content child HWND parented to the viewport
- controls parented to the content HWND

The viewport owns scrolling state and receives wheel/scroll messages. The content HWND has the full content extent and is moved by the negative scroll offset.

Maintain Rust-side state:
- content size
- viewport size
- offset x/y
- maximum offset x/y
- enabled axes
- page/range/position information

Reuse `ScrollWindow` where appropriate, but do not assume it alone provides generic clipping or scroll-info state. Add portable low-level wrappers for native scroll information where supported. Treat REAPER `CoolSB_*` functions as an optional host-specific adapter, not the core cross-platform contract.

Window-level scrolling may be exposed through `WindowSpec`, but it should reuse the same viewport/content abstraction. Enabling it changes native control parenting and therefore requires explicit rebinding support.

### Explicit scope boundaries
Included:
- pure geometry and layout contracts
- resize/docking relayout
- minimum-size negotiation
- stable logical control identity
- native viewport/content design for ScrollView
- wheel and scrollbar state model

Deferred:
- implementation before the next design discussion
- custom painting
- full intrinsic text/font measurement
- grid layout
- accessibility redesign
- cross-DAW layout portability
- assuming group boxes are native parents
- claiming ScrollView support without clipping and child-event tests

### Candidate implementation phases

1. **Contract phase**
   - Define geometry, sizing, constraints, padding, justification, and layout-node ownership.
2. **Pure engine phase**
   - Implement Fixed, single-axis Rows/Columns, Stack wrapping, and Box delegation with unit tests.
3. **Window policy phase**
   - Add layout-derived minimums and `WM_GETMINMAXINFO`; verify docked undersizing behavior.
4. **Gallery phase**
   - Replace manual gallery coordinates and validate resize/docking before native scrolling.
5. **Native ScrollView phase**
   - Add child-window creation, viewport/content lifecycle, scroll-info messages, clipping, rebinding, and window-level configuration.
6. **Integration verification phase**
   - Test floating, docking, redocking, resize, control events, scroll offsets, and HWND recreation.

### Relevant files
- `rea-rs/src/layout.rs` — proposed pure layout contracts, algorithms, and tests.
- `rea-rs/src/swell_gui.rs` — control rectangles, window lifecycle, relayout, minimum-size handling, and native integration.
- `rea-rs/src/lib.rs` — public exports.
- `rea-rs/src/reaper.rs` — window registry lifecycle only if new child-window ownership requires it.
- `low/src/swell_impl.rs` — hand-written child-window and native scrolling extensions.
- `low/src/raw.rs` — missing hand-written constants/types.
- `test/src/swell_gui.rs` — gallery and manual validation.

### Verification criteria
- Fixed layout never changes authored dimensions.
- Rows and Columns never wrap.
- Stack wraps only when explicitly selected.
- Box does not duplicate flow allocation semantics.
- Padding and spacing are deducted before allocation.
- Minimum constraints never cause a panic or resize feedback loop.
- Floating minimums work through `WM_GETMINMAXINFO`.
- Docked windows can shrink below layout minimums.
- ScrollView is not considered complete until viewport clipping, scroll commands, control events, and rebind cycles work on the supported native backends.
- Repeated resize and dock/float cycles produce the same geometry for the same available rectangle.

### Final recommendation
Do not expand the previous plan further yet. It should be retained as a backup. Before implementation, reconsider the fundamental model: perhaps the desired system is not a general-purpose widget layout tree, but a smaller constraint/anchoring mechanism, a declarative native-dialog description, or a host-managed coordinate policy. The next idea should be evaluated against the actual major problems before selecting APIs or implementing ScrollView.