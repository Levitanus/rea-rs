# 1.0.0 Public API Audit (incomplete release gate)

This ledger records release-surface dispositions. The 1.0.0 API is the canonical API: no compatibility aliases or deprecations. A change marked **breaking** must be propagated to every workspace caller and covered by tests before release. Native calls with no status result cannot honestly promise operation success; document them as request-only.

## Completed initial changes

| File | Public API | Disposition | Notes |
|---|---|---|---|
| `rea-rs/src/utils.rs` | `string_from_const_i8` | **Breaking: unsafe** | Raw pointer conversion is `unsafe`, has a non-null check, and documents pointer validity/NUL-termination requirements. FFI callers now put the unsafe contract at the call site. |
| `rea-rs/src/utils.rs` | `WithReaperPtr::with_valid_ptr` | **Safety fix** | Restores checked state for closure errors and unwinding panics before propagating/resuming. |
| `rea-rs/src/misc_types.rs` | `SampleAmount::from_time`, `SampleAmount::as_time` | **Breaking: `ReaperResult`** | Rejects zero sample rate and overflow; caller in `AudioAccessor::get_sample_block_raw` propagates conversion errors. |
| `rea-rs/src/misc_types.rs` | `GetLength::get_length` | **Breaking: `ReaperResult<Duration>`** | End-position inputs that precede the start position return an error instead of panicking; `Track::add_item`, `Track::add_midi_item`, and envelope automation-item APIs propagate it. |
| `rea-rs/src/misc_types.rs` | `TryFrom<f64> for Volume/Pan/PanLaw`, `TryFrom<i32> for PanLawMode` | **Breaking: fallible numeric conversion** | Rejects NaN/infinity, out-of-range pan/volume values, and unknown enum/pan-law inputs; all host-backed getters and workspace examples/tests use `TryFrom`. |
| `rea-rs/src/misc_types.rs`, `rea-rs/src/source.rs` | `Position`, `SourceOffset`, conversions | **Breaking representation change** | Positions store `SourceOffset` backed by `TimeDelta`, support negative project times, and have microsecond precision. Conversion to unsigned `Duration` is fallible. |
| `rea-rs/src/misc_types.rs` | `Measure.index: i32`, `Measure::from_index(i32, ...)` | **Breaking type/index correction** | Direct signed `i32` values, including zero, are passed to REAPER; no one-based translation. |
| `rea-rs/src/misc_enums.rs` | `VUMode::from_raw`, `TrackFolderState::from_raw` | **Breaking: `ReaperResult`** | Unknown host values and invalid folder depths return errors; track getters propagate them. |
| `rea-rs/src/midi.rs` | Typed `MidiMessage::from_raw` implementations and `MidiEventBuilder::next` | **Safety behavior** | Empty/truncated messages are rejected or end malformed event iteration rather than indexing/slicing/panicking; `NotationMessage::try_notation` exposes detailed parse errors; invalid text is lossily decoded for display. |
| `rea-rs/src/track.rs` | `Track::add_item` and `add_midi_item` | **Error propagation** | Validates length ranges and propagates item property setter errors instead of returning incorrectly ranged items. |
| `rea-rs/src/send.rs`, `rea-rs/src/track.rs` | Generic send constructors and `Track::{get_send,get_receive,get_hardware_send}` | **Breaking: `ReaperResult<Option<_>>`** | Invalid/stale send indices are distinct from pointer/host validation errors; `get_recieve` is replaced by canonical `get_receive`, with no compatibility alias. `TrackSend::create_new` now ties its lifetime to the source track and no longer transmutes references. |
| `rea-rs/src/project.rs`, `rea-rs/src/simple_functions.rs` | `Project::new`, `Reaper::{current_project,add_project_tab,perform_action}`, `Project::{record,glue_selected_items}` | **Breaking: fallible resource/host operations** | Missing current project, command-ID conversion and action dispatch errors are propagated. |
| `rea-rs/src/envelope.rs`, `rea-rs/src/item.rs` | Automation-item bounds and item free-mode geometry setters | **Breaking: validation errors** | Non-finite/out-of-range values return errors rather than asserting/panicking. |
| `rea-rs/src/take.rs` | `Take::{name,guid,channel_mode,select_all_midi_events,sort_midi}` | **Fallibility fix** | Unknown host metadata and using MIDI operations on audio takes produce errors instead of panics. |
| `rea-rs/src/take.rs` | `Take::{get_midi,midi_hash}` | **Buffer safety fix** | Rejects negative/undersized native buffer requests, checked-converts sizes, and validates the native MIDI byte count before truncating. |
| `rea-rs/src/utils.rs` | `string_from_buf` | **Boundary fix** | Accepts an exact-fit NUL-terminated output string instead of misclassifying a terminator in the final buffer byte as truncation. |
| `rea-rs/src/misc_types.rs` | `GUID::to_string` | **Breaking: `ReaperResult<String>`** | Formatting errors no longer silently become an empty GUID string; item/take/track setters and host envelope selectors propagate errors. |
| `rea-rs/src/project.rs` | `Project::new` | **Breaking: `ReaperResult<Project>`** | Missing current project now fails explicitly. The high-frequency `Reaper::current_project` convenience accessor retains its existing infallible shape and clearly expects REAPER to have an active project. |

## Native source compatibility policy

Use the latest upstream WDL and REAPER SDK snapshots for all supported REAPER
versions. The project intentionally has no per-version WDL/SDK selection or
commit pin because the user expects newer sources to retain backwards
compatibility. Keep snapshots checked in so ordinary builds do not require a
network fetch; update them only through the explicit source-refresh maintenance
feature and review the generated bindings. The integration runner separately
targets latest stable REAPER 7.82.
| `rea-rs/src/simple_functions.rs` | `Reaper::main_hwnd` | **Breaking: `ReaperResult`** | Null REAPER window handle is reported, not panicked. |
| `rea-rs/src/simple_functions.rs` | `Reaper::show_console_msg` | **Breaking: `ReaperResult<()>`** | Embedded NUL no longer logs and silently drops the message. All workspace callers either propagate or explicitly handle logging failure. |
| `rea-rs/src/simple_functions.rs` | `Reaper::get_action_name` | **Breaking: `ReaperResult<Option<String>>`** | Checked signed command-ID conversion and UTF-8 conversion. |
| `rea-rs/src/simple_functions.rs` | `Reaper::get_binary_directory` | **Breaking: `ReaperResult<String>`** | UTF-8/null conversion errors are propagated. |
| `rea-rs/src/simple_functions.rs` | `Reaper::get_global_automation_mode` | **Breaking: `ReaperResult<Option<AutomationMode>>`** | Unknown host enum value is reported. |
| `rea-rs/src/simple_functions.rs` | `Reaper::get_user_inputs` | **Safety/fallibility fix** | Validates buffer size and caption count, propagates CString conversion errors, and rejects a response with mismatched field count. |
| `rea-rs/src/simple_functions.rs` | `Reaper::{get_theme_color,set_theme_color}` | **Breaking: `ReaperResult<Color>`** | Converts CString and native `-1` sentinel failures to errors. `Color::try_from(ThemeColor)` replaces the fallible `From` conversion. |
| `rea-rs/src/color.rs` | `TryFrom<ThemeColor> for Color` | **Breaking: canonical fallible conversion** | Theme lookup can fail, so the infallible `From` implementation was removed. |
| `rea-rs/src/track.rs` | `Track::{from_name,from_guid}` | **Behavior change** | Propagates per-track metadata errors instead of treating them as a non-match. |
| `rea-rs/src/track.rs` | `Track::ui_element_rect`, `RazorEdit::{parse,FromStr,TryFrom<&str>}`, `Track::razor_edits` | **Breaking: fallible parsing** | Malformed host rectangle/Razor Edit data produces errors, not unwrap panics. |
| `rea-rs/src/item.rs` | `Item::{track,add_take,guid,solo_override,time_base}` | **Fallibility fix** | Null native resources and unknown enum/GUID values are returned as errors. |
| `rea-rs/src/project.rs` | `Project::with_current_project` | **Error handling fix** | Reports activation/restoration failures and attempts restoration after closure errors. |
| `rea-rs/src/project.rs` | `Project::record` | **Breaking: `anyhow::Result<()>`** | Project activation errors are no longer unwrapped into panics. |
| `rea-rs/src/simple_functions.rs` | `Reaper::get_user_inputs` | **Input validation fix** | Rejects invalid buffer/count sizes, embedded-NUL input, and mismatched host response fields. |
| `rea-rs-test/src/integration_test.rs` | Runner result interpretation | **Behavior change** | Reads a unique PASS/FAIL result-file handshake from the plugin; no longer maps process signal termination to 101 or infers test success/failure solely from REAPER's exit code. Timeout and missing/malformed result are explicit errors; host status is included in diagnostics. |
| `rea-rs-test/src/lib.rs` | Integration plugin result reporting | **Behavior change** | Atomically writes PASS/FAIL before the existing process exit. |
| `rea-rs-test/src/integration_test.rs` | Linux cache lookup | **Bug fix** | Checks the extracted REAPER executable under its install directory rather than joining an absolute `/reaper` path. |

## Remaining audit work — release is not yet signed off

The full per-method review is **not complete**. The module inventory below identifies coverage; it does not count as a disposition of every public symbol. A quick declaration search found over 1,000 public declarations across the high-level crate, so the release must not be represented as fully audited until each API item is reviewed and assigned one of: **breaking Result/TryFrom**, **safe unchanged**, **native API has no status**, or **documentation-only**. Prioritized candidate findings from the initial review:

- `misc_types.rs`: `Position` float construction/subtraction and `GetLength`; `Pan`, `PanLaw`, `PanLawMode`; `Measure::from_index`; signed/range conversion and serialization invariants.
- `midi.rs`: typed parsers/indexed accessors, `MidiEventBuilder`, text/notation/Bezier parsing, channel setters/constructors, note pairing.
- `track.rs`: `from_name`/`from_guid`, `ui_element_rect`, razor-edit parsing, send/receive getters, `add_item`, native index casts.
- `item.rs`, `take.rs`: null-returning constructors/lookups, enum/string conversion, MIDI buffer sizes/counts, audio-only MIDI calls, peak vector invariants.
- `source.rs`: time conversions and native lengths; `stretch_marker.rs` and `marker.rs`: error suppression in iterators and signed native count conversions.
- `project.rs`: project switching/restoration, undo lifecycle, `record`, project lookup, ignored statuses and partial mutations.
- `fx.rs`, `envelope.rs`, `send.rs`, `ext_state.rs`: ignored native statuses, assertions, stale indices, conversions and partial operations.
- `audio_accessor.rs`, `hardware_functions.rs`, `simple_functions.rs`: checked allocation arithmetic, input ranges, invalid host values, CString failures.
- `reaper.rs`, `control_surface.rs`, `socket.rs`: registration errors, lifecycle restoration, callback error reporting, background startup/poison handling.
- `swell_gui/*`: buffer termination, handle validity/ownership, ignored native statuses, fallible layout/drawing/geometry inputs, and callback unwind boundaries.

## Verification status

- `cargo fmt --all`, `cargo check --workspace`, and `cargo test --workspace --exclude reaper-test-extension-plugin` pass at the latest checkpoint. The workspace tests include unit and doctest coverage; 2 doctests and 1 test are intentionally ignored.
- `cargo test -p rea-rs-test --lib` passes 8 result-protocol/classifier tests, including PASS/FAIL, missing/malformed data, timeout, exit statuses 0/101/172/unexpected, and signal distinction.
- The Linux REAPER 7.82 hosted integration test passes after building and installing the current plug-in. A previous attempt exposed stale/improper assumptions around transport and VU-mode bits; these were corrected and the next host run passed.
- Build warnings remain in generated low-level bindings, workspace manifests and GUI test code; they have not all been classified as pre-existing versus release-introduced.
- **Outstanding release gates:** complete the symbol-by-symbol public API and native buffer/FFI audit; verify plugin FAIL reporting with an end-to-end injected failure; run package-content and `cargo publish --dry-run` checks for every publishable crate; test macOS/Windows CI; and select/test a stable MSRV before advertising one.
