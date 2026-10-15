# Unreleased

## Added
* `CreationContext::parent_hwnd` method for retrieving hwnd's of containers. 
There is a sense of reworking this API a bit
* `Take::insert_midi_event` and `Take::insert_midi_events` for easier inserting of single events into Take
* `Time::format_timestr`, `Time::parse_timestr`, `Time::format_timestr_len`, `Time::parse_timestr_len` and `TimeRange::format_timestr_len` as well as `TimeStringFormattingMode` enum for getting and parsing ruler time string representation.
* `Measure::get_ruler_index` for retriving visual measure index as it is displayed in ruler
* `quantize` module, which can be used for easy quantisation of any time units.

## Changed
* renamed `Position` to `Time`, as it represents now all reaper-related time
* reduced usage of `std::Duration` (left only in timer functions)
* all functions that previously used `Duration` or `GetLength` nor are using `Time`
* `TimeRange` type now corresponds to `Time` interval and will be future-developed ofr easier time-range manipulation (various boundaries, items manipulation etc.)
* for time selection and loop selection now is used `LoopRange` type
* `Project::get_time_range`, `Project::get_loop_selection` and `Project::get_time_selection` now are returning `ReaperResult<Option<LoopRange>>`, with being `None` if there is no active loop or time selection
* `Source::kength` now returns `SourceLength` enum, that represents or length in `Time` or length in quarter notes.

---

# 1.0.0 - 2026-10-08

This is the first stable release and contains breaking API changes. In
particular, fallible
host operations and conversions now return `ReaperResult`/`TryFrom`; project
positions are signed and use `SourceOffset` precision; measure indices use
REAPER's signed `i32` numbering directly (including zero); MIDI channel and
buffer validation is stricter; and SWELL-backed track geometry uses layout
types. There are no compatibility aliases or deprecated legacy signatures.

Publishing order: `rea-rs-low`, `rea-rs-macros`, `rea-rs`, then
`rea-rs-test`. Publish each crate only after its exact-version dependencies
are available on crates.io; the test plug-in package is not published.

## Changed

* Made invalid input, malformed host data, and fallible host calls report
    errors instead of panicking or silently substituting defaults.
* Added a PASS/FAIL file handshake to the REAPER integration-test runner and
    report host exit codes, signals, missing results, and timeouts separately.
* Refreshed vendored REAPER SDK and WDL sources from upstream.
* Aligned all publishable workspace crates and internal dependencies to 1.0.0.

## Fixed

* Corrected REAPER Linux cache detection and ensured the test extension is
    rebuilt before installation.
* Fixed native output-buffer boundary checks and MIDI event validation.

## Migration

Review the method-by-method audit before upgrading. Breaking changes include
fallible constructors/getters/setters, checked enum/numeric conversions,
signed position handling, direct `i32` measure indices, corrected
`get_receive` spelling, fallible send lookups, and explicit error propagation
for project switching, MIDI parsing, and resource validity.

---

# 0.2.0
## Changed

This refactoring introduces a major shift in the API design philosophy. The previous approach relied heavily on generic mutability markers and on storing parent-object references inside wrapper types such as Track, Item, Take, and related objects. That model has been simplified in favor of a more explicit and predictable ownership structure.

A broad pass was also made across the REAPER-facing API boundary. Functions that previously exposed raw-pointer-based access now return Result values, making error handling explicit and reducing the amount of implicit state around object validity. This includes methods such as get, is_dirty, make_current_project, play, stop, and many related helpers, which now propagate failures instead of assuming that the underlying object is always valid.

The old mutability abstraction has been removed. The previous ProbablyMutable, Mutable, and Immutable pattern, along with the generic Track<'a, T>-style wrappers, has been replaced by simpler concrete types. This also removes the old generic-based helper signatures from the position and measure APIs, such as the previous ppq_start, ppq_end, as_ppq, and from_ppq methods that depended on the old mutability marker model.

To reduce the overhead of repeated pointer validity checks, the codebase now uses with_checked and set_checked in a small number of carefully selected places. These helpers should be used sparingly and only in truly narrow, performance-sensitive paths where the tradeoff is intentional and well understood.

### Removed / replaced APIs

The following older API entry points were removed as part of the transition away from the old generic mutability model:

- Project::get_track_mut
- Project::get_item_mut
- Track::get_fx_mut
- The old generic mutability marker types: ProbablyMutable, Mutable, and Immutable
- The old generic wrapper style such as Track<'a, T>, Item<T>, and Take<T>

In practice, this means that code which previously relied on separate mutable and immutable variants, such as Track<Mutable> and Track<Immutable>, now needs to be expressed through the new, unified object model.

Example changes:

```rust
// before
pub fn is_dirty(&self) -> bool {
    unsafe { Reaper::get().low().IsProjectDirty(self.context().to_raw()) != 0 }
}

// after
pub fn is_dirty(&self) -> Result<bool, ReaRsError> {
    Ok(unsafe { Reaper::get().low().IsProjectDirty(self.get()?.as_ptr()) != 0 })
}
```

```rust
// before
pub struct Track<'a, T: ProbablyMutable> { /* ... */ }

// after
pub struct Track { /* ... */ }
```
