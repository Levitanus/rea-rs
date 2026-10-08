use crate::{
    ptr_wrappers::{Hwnd, MediaItem, MediaItemTake},
    ReaRsError, Reaper, ReaperResult, Take, WithReaperPtr,
};
use std::{
    ffi::{CStr, CString},
    ptr::NonNull,
};

/// Horizontal time coordinate used by the MIDI editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiEditorHorizontalOrigin {
    /// The left edge is expressed in take PPQ coordinates.
    Ppq(f64),
    /// The left edge is expressed in project seconds.
    ProjectTime(f64),
}

/// MIDI editor horizontal timebase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiEditorTimebase {
    /// Project-beat grid; horizontal positions are in take PPQ.
    ProjectBeats,
    /// Time follows project tempo changes; horizontal positions are seconds.
    ProjectSync,
    /// Horizontal positions are project seconds.
    ProjectTime,
    /// Source beat grid; horizontal positions are in take PPQ.
    SourceBeats,
    /// A future or undocumented REAPER timebase value.
    Unknown(i32),
}

impl MidiEditorTimebase {
    fn from_chunk_value(value: i32) -> Self {
        match value {
            0 => Self::ProjectBeats,
            1 => Self::ProjectSync,
            2 => Self::ProjectTime,
            4 => Self::SourceBeats,
            other => Self::Unknown(other),
        }
    }
    fn uses_ppq(self) -> bool {
        matches!(self, Self::ProjectBeats | Self::SourceBeats)
    }
}

/// MIDI editor display mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MidiEditorMode {
    PianoRoll,
    EventList,
}

/// Snapshot of the native MIDI editor's musical-to-client-pixel mapping.
///
/// This is read from REAPER's item state because the public MIDI editor API
/// currently has no view-transform getter. The state record is a REAPER
/// implementation detail; callers should treat this snapshot as best-effort
/// and reacquire it after scrolling, zooming, changing takes, or resizing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MidiEditorViewTransform {
    pub timebase: MidiEditorTimebase,
    pub horizontal_origin: MidiEditorHorizontalOrigin,
    /// Horizontal scale: pixels per PPQ in beat modes, pixels per second
    /// otherwise.
    pub horizontal_pixels_per_unit: f64,
    /// Raw CFGEDITVIEW vertical position. In the standard 0–127 piano roll,
    /// the top pitch is approximately `127 - vertical_position`.
    pub vertical_position: i32,
    /// Pixels per semitone in the standard piano-roll note view.
    pub vertical_pixels_per_pitch: f64,
}

impl MidiEditorViewTransform {
    /// Returns the client X coordinate for a take-PPQ position.
    pub fn x_for_ppq(&self, ppq: f64, take: &Take) -> ReaperResult<f64> {
        if !ppq.is_finite()
            || !self.horizontal_pixels_per_unit.is_finite()
            || self.horizontal_pixels_per_unit <= 0.0
        {
            return Err(ReaRsError::InvalidObject(
                "invalid MIDI editor X mapping",
            ));
        }
        let x = match self.horizontal_origin {
            MidiEditorHorizontalOrigin::Ppq(origin) => {
                (ppq - origin) * self.horizontal_pixels_per_unit
            }
            MidiEditorHorizontalOrigin::ProjectTime(origin) => {
                let take = take.get()?;
                let low = Reaper::get().low();
                let target_time = unsafe {
                    low.MIDI_GetProjTimeFromPPQPos(take.as_ptr(), ppq)
                };
                (target_time - origin) * self.horizontal_pixels_per_unit
            }
        };
        if x.is_finite() {
            Ok(x)
        } else {
            Err(ReaRsError::InvalidObject("invalid MIDI editor X mapping"))
        }
    }

    /// Returns the approximate client Y coordinate for a pitch row center.
    /// `note_area_top` is the client Y coordinate at the top of the piano-roll
    /// note rows (excluding any ruler/header area).
    pub fn y_for_pitch(
        &self,
        pitch: i32,
        note_area_top: f64,
    ) -> ReaperResult<f64> {
        if !(0..=127).contains(&pitch) {
            return Err(ReaRsError::InvalidObject(
                "MIDI pitch must be 0..=127",
            ));
        }
        let top_pitch = 127 - self.vertical_position;
        let y = note_area_top
            + (f64::from(top_pitch - pitch) + 0.5)
                * self.vertical_pixels_per_pitch;
        if y.is_finite() {
            Ok(y)
        } else {
            Err(ReaRsError::InvalidObject("invalid MIDI editor Y mapping"))
        }
    }

    /// Returns the take-PPQ coordinate at a client X coordinate.
    pub fn ppq_at_x(&self, x: f64, take: &Take) -> ReaperResult<f64> {
        if !x.is_finite()
            || !self.horizontal_pixels_per_unit.is_finite()
            || self.horizontal_pixels_per_unit <= 0.0
        {
            return Err(ReaRsError::InvalidObject(
                "invalid MIDI editor X mapping",
            ));
        }
        match self.horizontal_origin {
            MidiEditorHorizontalOrigin::Ppq(origin) => {
                Ok(origin + x / self.horizontal_pixels_per_unit)
            }
            MidiEditorHorizontalOrigin::ProjectTime(origin) => {
                let target_time = origin + x / self.horizontal_pixels_per_unit;
                let take = take.get()?;
                Ok(unsafe {
                    Reaper::get()
                        .low()
                        .MIDI_GetPPQPosFromProjTime(take.as_ptr(), target_time)
                })
            }
        }
    }

    /// Returns the approximate MIDI pitch at client Y in the standard
    /// unfiltered 0–127 piano-roll layout.
    pub fn pitch_at_y(&self, y: f64, note_area_top: f64) -> ReaperResult<f64> {
        if !y.is_finite()
            || !note_area_top.is_finite()
            || !self.vertical_pixels_per_pitch.is_finite()
            || self.vertical_pixels_per_pitch <= 0.0
        {
            return Err(ReaRsError::InvalidObject(
                "invalid MIDI editor Y mapping",
            ));
        }
        let top_pitch = 127 - self.vertical_position;
        Ok(f64::from(top_pitch)
            - ((y - note_area_top) / self.vertical_pixels_per_pitch)
            + 0.5)
    }
}

#[derive(Clone, Copy, Debug)]
struct RawMidiEditorView {
    start_ppq: f64,
    horizontal_zoom: f64,
    vertical_position: i32,
    vertical_zoom: f64,
    timebase: MidiEditorTimebase,
}

impl RawMidiEditorView {
    fn transform(self, take: &Take) -> ReaperResult<MidiEditorViewTransform> {
        let horizontal_origin = if self.timebase.uses_ppq() {
            MidiEditorHorizontalOrigin::Ppq(self.start_ppq)
        } else {
            let take = take.get()?;
            MidiEditorHorizontalOrigin::ProjectTime(unsafe {
                Reaper::get()
                    .low()
                    .MIDI_GetProjTimeFromPPQPos(take.as_ptr(), self.start_ppq)
            })
        };
        Ok(MidiEditorViewTransform {
            timebase: self.timebase,
            horizontal_origin,
            horizontal_pixels_per_unit: self.horizontal_zoom,
            vertical_position: self.vertical_position,
            vertical_pixels_per_pitch: self.vertical_zoom,
        })
    }
}

fn parse_midi_editor_view(
    item_state: &str,
    take_index: usize,
) -> ReaperResult<RawMidiEditorView> {
    // SWS's take parser normalizes the first inline NAME block as a synthetic
    // first TAKE. This parser needs only the corresponding start offsets.
    let mut take_starts = Vec::new();
    let mut pending_take_start = None;
    let mut offset = 0;
    for line in item_state.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("TAKE") {
            pending_take_start = Some(offset);
        } else if trimmed.starts_with("NAME ") || trimmed == "NAME" {
            take_starts.push(pending_take_start.take().unwrap_or(offset));
        }
        offset += line.len() + 1;
    }
    let Some(&start) = take_starts.get(take_index) else {
        return Err(ReaRsError::InvalidObject(
            "active take section not found in item state",
        ));
    };
    let end = take_starts
        .get(take_index + 1)
        .copied()
        .unwrap_or(item_state.len());
    let take_state = &item_state[start..end];

    let view_line = take_state
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("CFGEDITVIEW "))
        .ok_or(ReaRsError::InvalidObject(
            "CFGEDITVIEW not found in active take state",
        ))?;
    let view_fields: Vec<_> = view_line.split_whitespace().collect();
    if view_fields.len() < 5 {
        return Err(ReaRsError::InvalidObject("malformed CFGEDITVIEW"));
    }
    let parse_float = |index: usize, message| {
        view_fields[index]
            .parse::<f64>()
            .map_err(|_| ReaRsError::InvalidObject(message))
    };
    let start_ppq = parse_float(1, "invalid CFGEDITVIEW start")?;
    let horizontal_zoom =
        parse_float(2, "invalid CFGEDITVIEW horizontal zoom")?;
    let vertical_position = view_fields[3].parse::<i32>().map_err(|_| {
        ReaRsError::InvalidObject("invalid CFGEDITVIEW vertical position")
    })?;
    let vertical_zoom = parse_float(4, "invalid CFGEDITVIEW vertical zoom")?;

    let cfg_edit = take_state
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("CFGEDIT "))
        .ok_or(ReaRsError::InvalidObject(
            "CFGEDIT not found in active take state",
        ))?;
    let timebase = cfg_edit
        .split_whitespace()
        // SWS reads LineParser token 19 (1-based after CFGEDIT), which is
        // split_whitespace index 19 including the CFGEDIT keyword.
        .nth(19)
        .and_then(|value| value.parse::<i32>().ok())
        .map(MidiEditorTimebase::from_chunk_value)
        .ok_or(ReaRsError::InvalidObject("invalid CFGEDIT timebase"))?;

    if !start_ppq.is_finite()
        || !horizontal_zoom.is_finite()
        || horizontal_zoom <= 0.0
        || !vertical_zoom.is_finite()
        || vertical_zoom <= 0.0
    {
        return Err(ReaRsError::InvalidObject(
            "invalid MIDI editor view transform values",
        ));
    }
    Ok(RawMidiEditorView {
        start_ppq,
        horizontal_zoom,
        vertical_position,
        vertical_zoom,
        timebase,
    })
}

struct ReaperHeapString(NonNull<std::os::raw::c_char>);

impl Drop for ReaperHeapString {
    fn drop(&mut self) {
        unsafe {
            Reaper::get()
                .low()
                .FreeHeapPtr(self.0.as_ptr() as *mut std::ffi::c_void);
        }
    }
}

fn read_item_state(item: MediaItem) -> ReaperResult<String> {
    let low = Reaper::get().low();
    let empty = CString::new("").expect("empty C string is valid");
    let object = item.as_ptr() as *mut std::ffi::c_void;
    // Match SWS's SNM_ChunkParserPatcher state retrieval path.
    let state = unsafe { low.GetSetObjectState(object, empty.as_ptr()) };
    let state = NonNull::new(state)
        .ok_or(ReaRsError::UnsuccessfulOperation("GetSetObjectState"))?;
    let state = ReaperHeapString(state);
    Ok(unsafe { CStr::from_ptr(state.0.as_ptr()) }
        .to_string_lossy()
        .into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_inline_take_view() {
        let state = concat!(
            "<ITEM\n",
            "NAME inline\n",
            "<SOURCE MIDI\n",
            "CFGEDITVIEW 960 0.25 12 8\n",
            "CFGEDIT 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 4\n",
            ">\n>\n",
        );
        let parsed = parse_midi_editor_view(state, 0).unwrap();
        assert_eq!(parsed.start_ppq, 960.0);
        assert_eq!(parsed.horizontal_zoom, 0.25);
        assert_eq!(parsed.vertical_position, 12);
        assert_eq!(parsed.vertical_zoom, 8.0);
        assert_eq!(parsed.timebase, MidiEditorTimebase::SourceBeats);
    }

    #[test]
    fn parses_second_take_view_separately() {
        let state = concat!(
            "<ITEM\n",
            "TAKE\nNAME first\n<SOURCE MIDI\n",
            "CFGEDITVIEW 100 0.5 10 6\n",
            "CFGEDIT 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 0\n>\n",
            "TAKE\nNAME second\n<SOURCE MIDI\n",
            "CFGEDITVIEW 300 1.5 20 10\n",
            "CFGEDIT 0 0 0 0 0 0 0 0 1 0 0 0 0 0 0 0 0 0 2\n>\n>\n",
        );
        let parsed = parse_midi_editor_view(state, 1).unwrap();
        assert_eq!(parsed.start_ppq, 300.0);
        assert_eq!(parsed.horizontal_zoom, 1.5);
        assert_eq!(parsed.vertical_position, 20);
        assert_eq!(parsed.vertical_zoom, 10.0);
        assert_eq!(parsed.timebase, MidiEditorTimebase::ProjectTime);
    }
}

#[derive(Debug, PartialEq)]
pub struct MIDIEditor {
    hwnd: Hwnd,
    checked: bool,
}
impl WithReaperPtr for MIDIEditor {
    type Ptr = Hwnd;

    fn get_pointer(&self) -> Self::Ptr {
        self.hwnd
    }

    fn get(&self) -> Result<Self::Ptr, ReaRsError> {
        self.require_valid()
    }

    fn make_unchecked(&mut self) {
        self.checked = false
    }

    fn make_checked(&mut self) {
        self.checked = true
    }

    fn should_check(&self) -> bool {
        self.checked
    }
}
impl MIDIEditor {
    pub fn new(hwnd: Hwnd) -> Self {
        Self {
            hwnd,
            checked: true,
        }
    }

    /// Returns the editor's current display mode.
    pub fn mode(&self) -> ReaperResult<MidiEditorMode> {
        let mode = unsafe {
            Reaper::get().low().MIDIEditor_GetMode(self.get()?.as_ptr())
        };
        match mode {
            0 => Ok(MidiEditorMode::PianoRoll),
            1 => Ok(MidiEditorMode::EventList),
            _ => Err(ReaRsError::InvalidObject("invalid MIDI editor")),
        }
    }

    /// Reads the editor's current time/pitch-to-client-pixel transform.
    ///
    /// REAPER does not currently expose a documented numeric view-transform
    /// getter. This method reads `CFGEDITVIEW` from the active take's item
    /// state, following the same source used by SWS. This is a best-effort
    /// read of a REAPER implementation detail, not a stable serialized API.
    /// Call it again after scrolling, zooming, resizing, or changing takes.
    pub fn view_transform(&self) -> ReaperResult<MidiEditorViewTransform> {
        self.mode()?;
        let take = self.get_active_take()?;
        let item = take.parent_item()?;
        let take_pointer = take.get()?;
        let take_index_key =
            CString::new("IP_TAKENUMBER").expect("static string is valid");
        let take_index = unsafe {
            Reaper::get().low().GetMediaItemTakeInfo_Value(
                take_pointer.as_ptr(),
                take_index_key.as_ptr(),
            )
        };
        if !take_index.is_finite() || take_index < 0.0 {
            return Err(ReaRsError::InvalidObject(
                "invalid active MIDI take index",
            ));
        }
        let item_state = read_item_state(item.get()?)?;
        parse_midi_editor_view(&item_state, take_index as usize)?
            .transform(&take)
    }

    /// Enumerates the editor's native child windows in z-order.
    ///
    /// Returned wrappers are non-owning. Use this to inspect REAPER's current
    /// editor hierarchy without taking ownership of its controls.
    pub fn child_windows(
        &self,
    ) -> ReaperResult<impl Iterator<Item = ReaperResult<crate::ReaperWindow>>>
    {
        crate::ReaperWindow::from_hwnd_token(crate::ReaperHwnd::from_raw(
            self.get()?.as_ptr(),
        ))?
        .children()
    }

    /// Enumerates all descendant windows in native enumeration order.
    pub fn descendant_windows(
        &self,
    ) -> ReaperResult<impl Iterator<Item = ReaperResult<crate::ReaperWindow>>>
    {
        crate::ReaperWindow::from_hwnd_token(crate::ReaperHwnd::from_raw(
            self.get()?.as_ptr(),
        ))?
        .descendants()
    }

    /// Sends a MIDI-editor action command to this editor.
    ///
    /// Use command IDs from REAPER's MIDI Editor action section. Command IDs
    /// are REAPER-version-specific; this method intentionally does not encode
    /// particular scroll or zoom actions as stable library constants.
    pub fn on_command(&self, command_id: impl Into<u32>) -> ReaperResult<()> {
        let command_id = i32::try_from(command_id.into()).map_err(|_| {
            ReaRsError::InvalidObject("invalid MIDI editor command ID")
        })?;
        let handled = unsafe {
            Reaper::get()
                .low()
                .MIDIEditor_OnCommand(self.get()?.as_ptr(), command_id)
        };
        if handled {
            Ok(())
        } else {
            Err(ReaRsError::UnsuccessfulOperation("MIDIEditor_OnCommand"))
        }
    }

    pub fn get_active_take(&self) -> ReaperResult<Take> {
        let rpr = Reaper::get().low();
        let ptr = unsafe { rpr.MIDIEditor_GetTake(self.get()?.as_ptr()) };
        match MediaItemTake::new(ptr) {
            None => Err(ReaRsError::NullPtr("MIDI editor take")),
            Some(ptr) => Take::new(ptr, None),
        }
    }

    pub fn enum_takes(
        &self,
        editable_only: bool,
    ) -> impl Iterator<Item = Take> {
        let low = Reaper::get().low();
        let ptr = self.hwnd;
        let mut idx = 0;
        std::iter::from_fn(move || {
            let result = unsafe {
                low.MIDIEditor_EnumTakes(ptr.as_ptr(), idx, editable_only)
            };
            let take_ptr = MediaItemTake::new(result)?;
            idx += 1;
            Some(Take::new(take_ptr, None).expect("should be valid Take"))
        })
    }
}
