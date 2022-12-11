use crate::{
    errors::{ReaperError, ReaperStaticResult},
    utils::{as_c_str, as_string_mut, make_c_string_buf, WithNull},
    GetLength, KnowsProject, Mutable, Position, ProbablyMutable, Reaper,
    WithReaperPtr, GUID,
};
use int_enum::IntEnum;
use reaper_medium::TrackEnvelope;
use std::{
    ffi::CString, marker::PhantomData, mem::MaybeUninit, time::Duration,
};

/// Can be either TrackEnvelope, or TakeEnvelope
///
/// Main points of struct construction are:
/// - [crate::Track::get_envelope]
/// - [crate::Track::get_envelope_by_chunk] (or guid)
/// - [crate::Track::get_envelope_by_name] (I'm not sure it works.)
/// - [crate::TrackSend::get_envelope]
/// - [crate::FxParam::add_envelope]
/// - [crate::Take::get_envelope]
#[derive(Debug, PartialEq)]
pub struct Envelope<'a, P: KnowsProject, T: ProbablyMutable> {
    ptr: TrackEnvelope,
    parent: &'a P,
    should_check: bool,
    phantom: PhantomData<T>,
}
impl<'a, P: KnowsProject, T: ProbablyMutable> WithReaperPtr<'a>
    for Envelope<'a, P, T>
{
    type Ptr = TrackEnvelope;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.parent.project()).unwrap();
        self.ptr
    }
    fn make_unchecked(&mut self) {
        self.should_check = false;
    }
    fn make_checked(&mut self) {
        self.should_check = true;
    }
    fn should_check(&self) -> bool {
        self.should_check
    }
}
impl<'a, P: KnowsProject, T: ProbablyMutable> Envelope<'a, P, T> {
    pub fn new(ptr: TrackEnvelope, parent: &'a P) -> Self {
        Self {
            ptr,
            parent,
            should_check: true,
            phantom: PhantomData,
        }
    }
    pub fn guid(&self) -> GUID {
        let size = 50;
        let buf = make_c_string_buf(size);
        let ptr = buf.into_raw();
        let category = CString::new("GUID")
            .expect("Can not convert category to CString.");
        let result = unsafe {
            Reaper::get().low().GetSetEnvelopeInfo_String(
                self.get().as_ptr(),
                category.as_ptr(),
                ptr,
                false,
            )
        };
        if !result {
            panic!("Can't get GUID");
        }
        let guid_string =
            as_string_mut(ptr).expect("Can't convert ptr to String!");
        GUID::from_string(guid_string).expect("Can't convert string to GUID!")
    }

    pub fn get_automation_item(
        &'a self,
        index: usize,
    ) -> Option<AutomationItem<'a, P, T>> {
        if index >= self.n_automation_items() {
            return None;
        }
        Some(AutomationItem {
            envelope: self,
            index,
        })
    }

    pub fn name(&self) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        let result = unsafe {
            Reaper::get().low().GetEnvelopeName(
                self.get().as_ptr(),
                buf,
                size as i32,
            )
        };
        match result {
            false => panic!("Can not get envelope name!"),
            true => {
                as_string_mut(buf).expect("Can not convert name to string")
            }
        }
    }

    /// - 0 → no scaling,
    /// - 1 → fader scaling
    fn scaling_mode(&self) -> i32 {
        unsafe {
            Reaper::get()
                .low()
                .GetEnvelopeScalingMode(self.get().as_ptr())
        }
    }
    fn scale_to(&self, value: f64) -> f64 {
        Reaper::get()
            .low()
            .ScaleToEnvelopeMode(self.scaling_mode(), value)
    }
    fn scale_from(&self, value: f64) -> f64 {
        Reaper::get()
            .low()
            .ScaleFromEnvelopeMode(self.scaling_mode(), value)
    }

    /// Full Envelope state, as it written in project file.
    pub fn state_chunk(&self) -> String {
        let size = i32::MAX;
        let buf = make_c_string_buf(size as usize).into_raw();
        let result = unsafe {
            Reaper::get().low().GetEnvelopeStateChunk(
                self.get().as_ptr(),
                buf,
                size as i32,
                false,
            )
        };
        match result {
            false => panic!("Can not get envelope name!"),
            true => {
                as_string_mut(buf).expect("Can not convert name to string")
            }
        }
    }

    pub fn n_points(&self) -> usize {
        self.n_points_ex(None, false)
    }
    fn n_points_ex(
        &self,
        automation_item_index: Option<usize>,
        only_visible: bool,
    ) -> usize {
        let a_itm = automation_item_idx(only_visible, automation_item_index);
        unsafe {
            Reaper::get()
                .low()
                .CountEnvelopePointsEx(self.get().as_ptr(), a_itm)
                as usize
        }
    }

    pub fn n_automation_items(&self) -> usize {
        unsafe {
            Reaper::get()
                .low()
                .CountAutomationItems(self.get().as_ptr()) as usize
        }
    }

    pub fn get_point(
        &self,
        point_index: usize,
    ) -> ReaperStaticResult<EnvelopePoint> {
        self.get_point_ex(None, false, point_index)
    }

    fn get_point_ex(
        &self,
        automation_item_index: Option<usize>,
        only_visible: bool,
        point_index: usize,
    ) -> ReaperStaticResult<EnvelopePoint> {
        let a_itm = automation_item_idx(only_visible, automation_item_index);
        let mut time = MaybeUninit::zeroed();
        let mut value = MaybeUninit::zeroed();
        let mut shape = MaybeUninit::zeroed();
        let mut tension = MaybeUninit::zeroed();
        let mut selected = MaybeUninit::zeroed();
        let result = unsafe {
            Reaper::get().low().GetEnvelopePointEx(
                self.get().as_ptr(),
                a_itm,
                point_index as i32,
                time.as_mut_ptr(),
                value.as_mut_ptr(),
                shape.as_mut_ptr(),
                tension.as_mut_ptr(),
                selected.as_mut_ptr(),
            )
        };
        match result {
            true => unsafe {
                Ok(EnvelopePoint {
                    value: self.scale_from(value.assume_init()),
                    shape: EnvelopePointShape::from_int(shape.assume_init())
                        .expect(
                            "Can not convert result to envelope point shape",
                        ),
                    tension: tension.assume_init(),
                    selected: selected.assume_init(),
                })
            },
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not set envelope point!",
            )
            .into()),
        }
    }

    /// Get the last point before the given time.
    pub fn get_point_by_time(
        &self,
        position: impl Into<Position>,
    ) -> ReaperStaticResult<EnvelopePoint> {
        self.get_point_by_time_ex(None, false, position)
    }

    fn get_point_by_time_ex(
        &self,
        automation_item_index: Option<usize>,
        only_visible: bool,
        position: impl Into<Position>,
    ) -> ReaperStaticResult<EnvelopePoint> {
        let a_itm = automation_item_idx(only_visible, automation_item_index);
        let point_index = unsafe {
            Reaper::get().low().GetEnvelopePointByTimeEx(
                self.get().as_ptr(),
                a_itm,
                position.into().into(),
            )
        };
        if point_index < 0 {
            return Err(ReaperError::UnsuccessfulOperation(
                "Can not find point at the given time",
            ));
        }
        self.get_point_ex(
            automation_item_index,
            only_visible,
            point_index as usize,
        )
    }

    /// Get the effective envelope value at a given time position.
    ///
    /// `samples_requested` is how long the caller expects until the next call
    /// to `evaluate` (often, the buffer block size).
    ///
    /// See [Envelope::scaling_mode].
    pub fn evaluate(
        &self,
        position: Position,
        samplerate: u32,
        samples_requested: usize,
    ) -> EnvelopeEvaluateResult {
        let (mut value, mut dvds, mut ddvds, mut dddvds) = (
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
            MaybeUninit::zeroed(),
        );
        unsafe {
            let result = Reaper::get().low().Envelope_Evaluate(
                self.get().as_ptr(),
                position.as_duration().as_secs_f64(),
                samplerate as f64,
                samples_requested as i32,
                value.as_mut_ptr(),
                dvds.as_mut_ptr(),
                ddvds.as_mut_ptr(),
                dddvds.as_mut_ptr(),
            );
            EnvelopeEvaluateResult {
                position,
                valid_for: result as usize,
                value: self.scale_from(value.assume_init()),
                first_derivative: dvds.assume_init(),
                second_derivative: ddvds.assume_init(),
                third_derivative: dddvds.assume_init(),
            }
        }
    }

    /// Get value, as it written in GUI
    pub fn format_value(&self, value: f64) -> String {
        let size = 100;
        let buf = make_c_string_buf(size).into_raw();
        unsafe {
            Reaper::get().low().Envelope_FormatValue(
                self.get().as_ptr(),
                value,
                buf,
                size as i32,
            );
        }
        as_string_mut(buf).expect("Can not convert value to string")
    }

    /// Get info value by string key. Should not be used in 99% cases.
    ///
    /// Is public only for the case of pointers information retrieval, if
    /// suddenly needed.
    ///
    /// # unimplemented categories:
    ///
    /// - P_TRACK : MediaTrack * : parent track pointer (if any)
    /// - P_DESTTRACK : MediaTrack * : destination track pointer, if on a send
    /// - P_ITEM : MediaItem * : parent item pointer (if any)
    /// - P_TAKE : MediaItem_Take * : parent take pointer (if any)
    pub fn get_info_value(&self, category: impl Into<String>) -> f64 {
        let mut category = category.into();
        unsafe {
            Reaper::get().low().GetEnvelopeInfo_Value(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
            )
        }
    }

    /// Is envelope relative to track send\receive\hw_out
    ///
    /// None if not.
    pub fn send_info(&self) -> Option<EnvelopeSendInfo> {
        let idx = self.get_info_value("I_SEND_IDX") as usize;
        if idx > 0 {
            return EnvelopeSendInfo::TrackSend(idx - 1).into();
        }
        let idx = self.get_info_value("I_HWOUT_IDX") as usize;
        if idx > 0 {
            return EnvelopeSendInfo::HardwareOut(idx - 1).into();
        }
        let idx = self.get_info_value("I_RECV_IDX") as usize;
        if idx > 0 {
            return EnvelopeSendInfo::TrackReceive(idx - 1).into();
        }
        None
    }

    /// current envelope automation state
    pub fn automation_state(&self) -> EnvelopeAutomationFlags {
        EnvelopeAutomationFlags::from_bits_truncate(unsafe {
            Reaper::get().low().GetEnvelopeUIState(self.get().as_ptr()) as u8
        })
    }

    /// Y offset of envelope relative to parent track
    ///
    /// (may be separate lane or overlap with track contents)
    pub fn tcp_y_offset(&self) -> usize {
        self.get_info_value("I_TCPY") as usize
    }
    /// Y offset of envelope relative to parent track, exclusive of padding
    pub fn tcp_y_offset_wo_padding(&self) -> usize {
        self.get_info_value("I_TCPY_USED") as usize
    }
    /// visible height of envelope
    pub fn tcp_height(&self) -> usize {
        self.get_info_value("I_TCPH") as usize
    }
    /// visible height of envelope, exclusive of padding
    pub fn tcp_height_wo_padding(&self) -> usize {
        self.get_info_value("I_TCPH_USED") as usize
    }
}

fn automation_item_idx(only_visible: bool, a_itm: Option<usize>) -> i32 {
    let a_itm = match a_itm {
        None => -1,
        Some(index) => index as i32,
    };
    match only_visible {
        false => a_itm | 0x10000000,
        true => a_itm,
    }
}

impl<'a, P: KnowsProject> Envelope<'a, P, Mutable> {
    /// Insert new automation item.
    ///
    /// if `pool_id` > 0 automation item will be a new instance of that pool
    /// (which will be created as an empty instance if it does not exist
    ///
    /// Otherwise, All underlying points will be collected.
    pub fn add_automation_item(
        &'a mut self,
        pool_id: usize,
        position: Position,
        length: impl GetLength,
    ) -> AutomationItem<'a, P, Mutable> {
        let length: f64 = length.get_length(position).as_secs_f64();
        let position: f64 = position.into();
        let pool_id = match pool_id {
            0 => -1,
            x => x as i32,
        };
        let index = unsafe {
            Reaper::get().low().InsertAutomationItem(
                self.get().as_ptr(),
                pool_id,
                position,
                length,
            )
        };
        AutomationItem {
            envelope: self,
            index: index as usize,
        }
    }

    /// Probably, if `position == None` — it can lead to moving point to the
    /// beginning
    pub fn set_point(
        &mut self,
        point_index: usize,
        position: Option<Position>,
        point: EnvelopePoint,
        sort: bool,
    ) -> ReaperStaticResult<()> {
        self.set_point_ex(None, false, point_index, position, point, sort)
    }
    fn set_point_ex(
        &self,
        automation_item_index: Option<usize>,
        only_visible: bool,
        point_index: usize,
        position: Option<Position>,
        point: EnvelopePoint,
        sort: bool,
    ) -> ReaperStaticResult<()> {
        let mut sort = MaybeUninit::new(!sort);
        let a_itm = automation_item_idx(only_visible, automation_item_index);
        let mut time = match position {
            Some(pos) => MaybeUninit::new(pos.as_duration().as_secs_f64()),
            None => MaybeUninit::zeroed(),
        };
        let mut value = MaybeUninit::new(self.scale_to(point.value));
        let mut shape = MaybeUninit::new(point.shape.int_value());
        let mut tension = MaybeUninit::new(point.tension.into());
        let mut selected = MaybeUninit::new(point.selected);
        let result = unsafe {
            Reaper::get().low().SetEnvelopePointEx(
                self.get().as_ptr(),
                a_itm,
                point_index as i32,
                time.as_mut_ptr(),
                value.as_mut_ptr(),
                shape.as_mut_ptr(),
                tension.as_mut_ptr(),
                selected.as_mut_ptr(),
                sort.as_mut_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not set envelope point!",
            )
            .into()),
        }
    }

    pub fn insert_point(
        &mut self,
        position: Position,
        point: EnvelopePoint,
        sort: bool,
    ) -> ReaperStaticResult<()> {
        self.insert_point_ex(None, false, position, point, sort)
    }
    fn insert_point_ex(
        &self,
        automation_item_index: Option<usize>,
        only_visible: bool,
        position: Position,
        point: EnvelopePoint,
        sort: bool,
    ) -> ReaperStaticResult<()> {
        let mut sort = MaybeUninit::new(!sort);
        let a_itm = automation_item_idx(only_visible, automation_item_index);
        let result = unsafe {
            Reaper::get().low().InsertEnvelopePointEx(
                self.get().as_ptr(),
                a_itm,
                position.into(),
                self.scale_to(point.value),
                point.shape.int_value(),
                point.tension.into(),
                point.selected,
                sort.as_mut_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not insert envelope point!",
            )
            .into()),
        }
    }

    pub fn delete_point(&mut self, index: usize) -> ReaperStaticResult<()> {
        self.delete_point_ex(None, false, index)
    }
    fn delete_point_ex(
        &self,
        automation_item_index: Option<usize>,
        only_visible: bool,
        index: usize,
    ) -> ReaperStaticResult<()> {
        let a_itm = automation_item_idx(only_visible, automation_item_index);
        let result = unsafe {
            Reaper::get().low().DeleteEnvelopePointEx(
                self.get().as_ptr(),
                a_itm,
                index as i32,
            )
        };
        match result {
            true => Ok(()),
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not delete envelope point!",
            )
            .into()),
        }
    }

    pub fn delete_point_range(
        &mut self,
        start: impl Into<Position>,
        end: impl Into<Position>,
    ) -> ReaperStaticResult<()> {
        self.delete_point_range_ex(None, false, start, end)
    }
    fn delete_point_range_ex(
        &self,
        automation_item_index: Option<usize>,
        only_visible: bool,
        start: impl Into<Position>,
        end: impl Into<Position>,
    ) -> ReaperStaticResult<()> {
        let a_itm = automation_item_idx(only_visible, automation_item_index);
        let result = unsafe {
            Reaper::get().low().DeleteEnvelopePointRangeEx(
                self.get().as_ptr(),
                a_itm,
                start.into().into(),
                end.into().into(),
            )
        };
        match result {
            true => Ok(()),
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not delete envelope points!",
            )
            .into()),
        }
    }

    pub fn sort_points(&mut self) {
        self.sort_points_ex(None)
    }
    fn sort_points_ex(&self, automation_item_index: Option<usize>) {
        let a_itm = match automation_item_index {
            Some(index) => index as i32,
            None => -1,
        };
        unsafe {
            Reaper::get()
                .low()
                .Envelope_SortPointsEx(self.get().as_ptr(), a_itm);
        }
    }

    /// Full Envelope state, as it written in project file.
    pub fn set_state_chunk(
        &mut self,
        state: impl Into<String>,
        with_undo: bool,
    ) -> ReaperStaticResult<()> {
        let mut state = state.into();
        let result = unsafe {
            Reaper::get().low().SetEnvelopeStateChunk(
                self.get().as_ptr(),
                as_c_str(state.with_null()).as_ptr(),
                with_undo,
            )
        };
        match result {
            true => Ok(()),
            false => Err(ReaperError::UnsuccessfulOperation(
                "Can not set envelope state",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopePoint {
    pub value: f64,
    pub shape: EnvelopePointShape,
    // from -1.0 to 1.0
    pub tension: f64,
    pub selected: bool,
}
impl EnvelopePoint {
    pub fn new(
        value: f64,
        shape: EnvelopePointShape,
        tension: f64,
        selected: bool,
    ) -> Self {
        Self {
            value,
            shape,
            tension,
            selected,
        }
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntEnum)]
pub enum EnvelopePointShape {
    Linear = 0,
    Square = 1,
    SlowStartEnd = 2,
    FastStart = 3,
    FastEnd = 4,
    Beizer = 5,
}

bitflags::bitflags! {
    /// Current envelope automation state
    pub struct EnvelopeAutomationFlags:u8{
        const PLAY_BACK=1;
        const WRITE=2;
        const HAD_CHANGED=4;
    }
}

/// Returned by [Envelope::evaluate].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopeEvaluateResult {
    position: Position,
    /// How many samples beyond that time position that the returned values
    /// are valid.
    pub valid_for: usize,
    pub value: f64,
    /// change in value per sample
    pub first_derivative: f64,
    pub second_derivative: f64,
    pub third_derivative: f64,
}

/// If Envelope corresponds to send or receive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeSendInfo {
    TrackSend(usize),
    TrackReceive(usize),
    HardwareOut(usize),
}

#[derive(Debug)]
pub struct AutomationItem<'a, P: KnowsProject, T: ProbablyMutable> {
    envelope: &'a Envelope<'a, P, T>,
    index: usize,
}
impl<'a, P: KnowsProject, T: ProbablyMutable> AutomationItem<'a, P, T> {
    pub fn index(&self) -> usize {
        self.index
    }
    fn envelope(&self) -> &Envelope<'a, P, T> {
        self.envelope
    }

    pub fn n_points(&self, only_visible: bool) -> usize {
        self.envelope()
            .n_points_ex(Some(self.index()), only_visible)
    }

    /// if `only_visible == true`, points will be presented as they are seen in
    /// project, including loop, if enabled.
    ///
    /// otherwise — raw representation, including those, that are truncated by
    /// item bounds.
    pub fn get_point(
        &self,
        only_visible: bool,
        point_index: usize,
    ) -> ReaperStaticResult<EnvelopePoint> {
        self.envelope().get_point_ex(
            Some(self.index()),
            only_visible,
            point_index,
        )
    }

    /// Get the last point before the given time.
    ///
    /// if `only_visible == true`, points will be presented as they are seen in
    /// project, including loop, if enabled.
    ///
    /// otherwise — raw representation, including those, that are truncated by
    /// item bounds.
    pub fn get_point_by_time(
        &self,
        only_visible: bool,
        position: impl Into<Position>,
    ) -> ReaperStaticResult<EnvelopePoint> {
        self.envelope().get_point_by_time_ex(
            Some(self.index()),
            only_visible,
            position,
        )
    }

    fn get_info_value(&self, category: impl Into<String>) -> f64 {
        let mut category = category.into();
        unsafe {
            Reaper::get().low().GetSetAutomationItemInfo(
                self.envelope().get().as_ptr(),
                self.index() as i32,
                as_c_str(category.with_null()).as_ptr(),
                0.0,
                false,
            )
        }
    }

    /// automation item pool ID (as an integer)
    ///
    /// edits are propagated to all other automation items that share a pool ID
    pub fn pool_id(&self) -> usize {
        self.get_info_value("D_POOL_ID") as usize
    }

    pub fn position(&self) -> Position {
        self.get_info_value("D_POSITION").into()
    }

    pub fn length(&self) -> Duration {
        Duration::from_secs_f64(self.get_info_value("D_LENGTH"))
    }

    pub fn start_offset(&self) -> Duration {
        Duration::from_secs_f64(self.get_info_value("D_STARTOFFS"))
    }

    pub fn play_rate(&self) -> f64 {
        self.get_info_value("D_PLAYRATE")
    }

    pub fn base_line(&self) -> f64 {
        self.get_info_value("D_BASELINE")
    }

    pub fn amplitude(&self) -> f64 {
        self.get_info_value("D_AMPLITUDE")
    }

    pub fn is_looped(&self) -> bool {
        self.get_info_value("D_LOOPSRC") != 0.0
    }

    pub fn is_selected(&self) -> bool {
        self.get_info_value("D_UISEL") != 0.0
    }
    pub fn pool_length_in_quarters(&self) -> f64 {
        self.get_info_value("D_POOL_QNLEN")
    }
}

impl<'a, P: KnowsProject> AutomationItem<'a, P, Mutable> {
    fn set_info_value(&self, category: impl Into<String>, value: f64) {
        let mut category = category.into();
        unsafe {
            Reaper::get().low().GetSetAutomationItemInfo(
                self.envelope().get().as_ptr(),
                self.index() as i32,
                as_c_str(category.with_null()).as_ptr(),
                value,
                true,
            );
        }
    }

    /// automation item pool ID (as an integer)
    ///
    /// edits are propagated to all other automation items that share a pool ID
    pub fn set_pool_id(&mut self, id: usize) {
        self.set_info_value("D_POOL_ID", id as f64)
    }

    pub fn set_position(&mut self, position: Position) {
        self.set_info_value("D_POSITION", position.into())
    }

    pub fn set_length(&mut self, length: impl GetLength) {
        self.set_info_value(
            "D_LENGTH",
            length.get_length(self.position()).as_secs_f64(),
        )
    }

    pub fn set_play_rate(&mut self, rate: f64) {
        assert!(rate > 0.0);
        self.set_info_value("D_PLAYRATE", rate)
    }

    /// Whatever this means, it should be in range of `0.0 .. 1.0`
    ///
    /// base line seems to work from API (I can set and read it). But it is not
    /// set in interface, and didn't affected to points in tests.
    pub fn set_base_line(&mut self, base_line: f64) {
        assert!((0.0..1.0).contains(&base_line));
        self.set_info_value("D_BASELINE", base_line)
    }

    /// Amplitude should be in range of `-1.0 .. 1.0`
    pub fn set_amplitude(&mut self, amplitude: f64) {
        assert!((-1.0..1.0).contains(&amplitude));
        self.set_info_value("D_AMPLITUDE", amplitude)
    }

    pub fn set_start_offset(&mut self, offset: Duration) {
        self.set_info_value("D_STARTOFFS", offset.as_secs_f64())
    }

    pub fn set_looped(&mut self, looped: bool) {
        self.set_info_value("D_LOOPSRC", looped as i32 as f64)
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.set_info_value("D_UISEL", selected as i32 as f64)
    }

    /// automation item pooled source length in quarter notes
    ///
    /// (setting will affect all pooled instances)
    pub fn set_pool_length_in_quarters(
        &mut self,
        pool_length_in_quarters: f64,
    ) {
        self.set_info_value("D_POOL_QNLEN", pool_length_in_quarters)
    }

    /// if `only_visible == true`, points will be presented as they are seen in
    /// project, including loop, if enabled.
    ///
    /// otherwise — raw representation, including those, that are truncated by
    /// item bounds.
    ///
    /// Probably, if `position == None` — it can lead to moving point to the
    /// beginning
    pub fn set_point(
        &mut self,
        only_visible: bool,
        point_index: usize,
        position: Option<Position>,
        point: EnvelopePoint,
        sort: bool,
    ) -> ReaperStaticResult<()> {
        let index = self.index();
        self.envelope().set_point_ex(
            Some(index),
            only_visible,
            point_index,
            position,
            point,
            sort,
        )
    }

    /// if `only_visible == true`, points will be presented as they are seen in
    /// project, including loop, if enabled.
    ///
    /// otherwise — raw representation, including those, that are truncated by
    /// item bounds.
    pub fn insert_point(
        &mut self,
        only_visible: bool,
        position: Position,
        point: EnvelopePoint,
        sort: bool,
    ) -> ReaperStaticResult<()> {
        let index = self.index();
        self.envelope().insert_point_ex(
            Some(index),
            only_visible,
            position,
            point,
            sort,
        )
    }

    /// if `only_visible == true`, points will be presented as they are seen in
    /// project, including loop, if enabled.
    ///
    /// otherwise — raw representation, including those, that are truncated by
    /// item bounds.
    pub fn delete_point(
        &mut self,
        only_visible: bool,
        index: usize,
    ) -> ReaperStaticResult<()> {
        let self_index = self.index();
        self.envelope()
            .delete_point_ex(Some(self_index), only_visible, index)
    }

    /// if `only_visible == true`, points will be presented as they are seen in
    /// project, including loop, if enabled.
    ///
    /// otherwise — raw representation, including those, that are truncated by
    /// item bounds.
    pub fn delete_point_range(
        &mut self,
        only_visible: bool,
        start: impl Into<Position>,
        end: impl Into<Position>,
    ) -> ReaperStaticResult<()> {
        let index = self.index();
        self.envelope().delete_point_range_ex(
            Some(index),
            only_visible,
            start,
            end,
        )
    }

    pub fn sort_points(&mut self) {
        let index = self.index();
        self.envelope().sort_points_ex(Some(index))
    }
}
