use crate::{
    errors::{ReaperError, ReaperStaticResult},
    utils::{as_c_str, as_c_string, as_string_mut},
    utils::{make_c_string_buf, WithNull},
    Color, Immutable, KnowsProject, Mutable, Position, ProbablyMutable,
    Project, Reaper, Take, TimeMode, Track, Volume, WithReaperPtr, GUID,
};
use int_enum::IntEnum;
use reaper_medium::{MediaItem, MediaItemTake, MediaTrack};
use serde_derive::{Deserialize, Serialize};
use std::{marker::PhantomData, ptr::NonNull, time::Duration};

#[derive(Debug, PartialEq)]
pub struct Item<'a, T: ProbablyMutable> {
    project: &'a Project,
    ptr: MediaItem,
    should_check: bool,
    phantom_mut: PhantomData<T>,
}
impl<'a, T: ProbablyMutable> WithReaperPtr<'a> for Item<'a, T> {
    type Ptr = MediaItem;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.project).unwrap();
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
impl<'a, T: ProbablyMutable> KnowsProject for Item<'a, T> {
    fn project(&self) -> &'a Project {
        self.project
    }
}
impl<'a, T: ProbablyMutable> Item<'a, T> {
    pub fn new(project: &'a Project, ptr: MediaItem) -> Self {
        Self {
            project,
            ptr,
            should_check: true,
            phantom_mut: PhantomData,
        }
    }
    pub fn track(&self) -> Track<Immutable> {
        let ptr = unsafe {
            Reaper::get().low().GetMediaItem_Track(self.get().as_ptr())
        };
        match MediaTrack::new(ptr) {
            None => panic!("Got null ptr! Maybe track is deleted?"),
            Some(ptr) => Track::new(self.project, ptr),
        }
    }
    pub fn get_take(&'a self, index: usize) -> Option<Take<'a, Immutable>> {
        let ptr = self.get_take_ptr(index)?;
        Some(Take::new(ptr, unsafe { std::mem::transmute(self) }))
    }
    fn get_take_ptr(&self, index: usize) -> Option<MediaItemTake> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetTake(self.get().as_ptr(), index as i32)
        };
        let ptr = NonNull::new(ptr);
        match ptr {
            None => None,
            x => x,
        }
    }
    pub fn active_take(&'a self) -> Take<'a, Immutable> {
        let ptr = self
            .active_take_ptr()
            .expect("NullPtr, probably, item is deleted");
        Take::<Immutable>::new(ptr, unsafe { std::mem::transmute(self) })
    }
    fn active_take_ptr(&self) -> Option<MediaItemTake> {
        let ptr =
            unsafe { Reaper::get().low().GetActiveTake(self.get().as_ptr()) };
        let ptr = NonNull::new(ptr);
        match ptr {
            None => None,
            x => x,
        }
    }

    pub fn is_selected(&self) -> bool {
        unsafe { Reaper::get().low().IsMediaItemSelected(self.get().as_ptr()) }
    }

    fn get_info_value(&self, category: impl Into<String>) -> f64 {
        let mut category = category.into();
        unsafe {
            Reaper::get().low().GetMediaItemInfo_Value(
                self.get().as_ptr(),
                as_c_str(&category.with_null()).as_ptr(),
            )
        }
    }

    pub fn position(&self) -> Position {
        self.get_info_value("D_POSITION").into()
    }
    pub fn length(&self) -> Duration {
        Duration::from_secs_f64(self.get_info_value("D_LENGTH"))
    }
    pub fn end_position(&self) -> Position {
        self.position() + self.length().into()
    }
    pub fn is_muted(&self) -> bool {
        self.get_info_value("B_MUTE") != 0.0
    }
    /// muted (ignores solo). setting this value will not affect
    /// [Item::solo_override].
    pub fn mute_actual(&self) -> bool {
        self.get_info_value("B_MUTE_ACTUAL") != 0.0
    }
    /// Basically, a way to solo particular item.
    ///
    /// This function will not override the same parameters on other items
    pub fn solo_override(&self) -> ItemSoloOverride {
        ItemSoloOverride::from_int(self.get_info_value("C_MUTE_SOLO") as i32)
            .expect("can not convert value to item solo override")
    }
    pub fn is_looped(&self) -> bool {
        self.get_info_value("B_LOOPSRC") != 0.0
    }
    pub fn all_takes_play(&self) -> bool {
        self.get_info_value("B_ALLTAKESPLAY") != 0.0
    }
    pub fn time_base(&self) -> TimeMode {
        TimeMode::from_int(self.get_info_value("C_BEATATTACHMODE") as i32)
            .expect("Can not convert balue to time mode")
    }
    pub fn auto_stretch(&self) -> bool {
        self.get_info_value("C_AUTOSTRETCH") != 0.0
    }
    pub fn locked(&self) -> bool {
        self.get_info_value("C_LOCK") != 0.0
    }
    pub fn volume(&self) -> Volume {
        Volume::from(self.get_info_value("D_VOL"))
    }
    pub fn snap_offset(&self) -> Duration {
        Duration::from_secs_f64(self.get_info_value("D_SNAPOFFSET"))
    }
    pub fn fade_in(&self) -> ItemFade {
        let length =
            Duration::from_secs_f64(self.get_info_value("D_FADEINLEN"));
        let curve = self.get_info_value("D_FADEINDIR");
        let shape = ItemFadeShape::from_int(
            self.get_info_value("C_FADEINSHAPE") as i32,
        )
        .unwrap();
        let auto_fade_length = self.get_info_value("D_FADEINLEN_AUTO") != 0.0;
        ItemFade {
            length,
            curve,
            shape,
            auto_fade_length,
        }
    }
    pub fn fade_out(&self) -> ItemFade {
        let length =
            Duration::from_secs_f64(self.get_info_value("D_FADEOUTLEN"));
        let curve = self.get_info_value("D_FADEOUTDIR");
        let shape = ItemFadeShape::from_int(
            self.get_info_value("C_FADEOUTSHAPE") as i32,
        )
        .unwrap();
        let auto_fade_length = self.get_info_value("D_FADEOUTLEN_AUTO") != 0.0;
        ItemFade {
            length,
            curve,
            shape,
            auto_fade_length,
        }
    }
    pub fn group_id(&self) -> usize {
        self.get_info_value("I_GROUPID") as usize
    }
    /// Y-position (relative to top of track) in pixels
    pub fn y_pos_relative(&self) -> usize {
        self.get_info_value("I_LASTY") as usize
    }
    /// Y-position (relative to top of track) in pixels when track is in free
    /// mode
    ///
    /// 0 → top of track, 1 → bottom of track (will never be 1)
    pub fn y_pos_free_mode(&self) -> f64 {
        self.get_info_value("F_FREEMODE_Y")
    }
    /// if None → default.
    pub fn color(&self) -> Option<Color> {
        let raw = self.get_info_value("I_CUSTOMCOLOR") as i32;
        if raw == 0 {
            return None;
        }
        Some(Color::from_native(raw & 0xffffff))
    }
    /// Height in pixels, when track is in free mode
    ///
    /// 0 → no height (will never be), 1 → full track.
    pub fn height_free_mode(&self) -> f64 {
        self.get_info_value("F_FREEMODE_H")
    }
    /// height in pixels
    pub fn height(&self) -> usize {
        self.get_info_value("I_LASTH") as usize
    }

    pub fn n_takes(&self) -> usize {
        unsafe {
            Reaper::get()
                .low()
                .GetMediaItemNumTakes(self.get().as_ptr()) as usize
        }
    }

    fn get_info_string(
        &self,
        category: impl Into<String>,
        buf_size: usize,
    ) -> ReaperStaticResult<String> {
        let mut category = category.into();
        let buf = make_c_string_buf(buf_size).into_raw();
        let result = unsafe {
            Reaper::get().low().GetSetMediaItemInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                buf,
                false,
            )
        };
        match result {
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not get value!"))
            }
            true => {
                Ok(as_string_mut(buf).expect("can not convert buf to string"))
            }
        }
    }
    pub fn notes(
        &self,
        buf_size: impl Into<Option<u32>>,
    ) -> ReaperStaticResult<String> {
        let size = buf_size.into().unwrap_or(1024);
        self.get_info_string("P_NOTES", size as usize)
    }
    pub fn guid(&self) -> GUID {
        let guid_str = self
            .get_info_string("GUID", 50)
            .expect("Can not get GUID string");
        GUID::from_string(guid_str)
            .expect("Can not convert GUID string to GUID")
    }
}
impl<'a> Item<'a, Mutable> {
    pub fn add_take(&mut self) -> Take<Mutable> {
        let ptr = unsafe {
            Reaper::get().low().AddTakeToMediaItem(self.get().as_ptr())
        };
        match MediaItemTake::new(ptr) {
            None => panic!("can not make Take"),
            Some(ptr) => Take::new(ptr, self),
        }
    }
    pub fn get_take_mut(
        &'a mut self,
        index: usize,
    ) -> Option<Take<'a, Mutable>> {
        let ptr = self.get_take_ptr(index)?;
        Some(Take::new(ptr, self))
    }
    pub fn active_take_mut(&'a mut self) -> Take<'a, Mutable> {
        let ptr = self.active_take_ptr().unwrap();
        Take::new(ptr, self)
    }

    pub fn set_position(&mut self, position: impl Into<Position>) {
        unsafe {
            Reaper::get().low().SetMediaItemPosition(
                self.get().as_ptr(),
                position.into().into(),
                true,
            );
        }
    }
    pub fn set_length(&mut self, length: Duration) {
        unsafe {
            Reaper::get().low().SetMediaItemLength(
                self.get().as_ptr(),
                length.as_secs_f64(),
                true,
            );
        }
    }
    pub fn set_end_position(&mut self, end_position: impl Into<Position>) {
        let length: Duration = (end_position.into() - self.position()).into();
        unsafe {
            Reaper::get().low().SetMediaItemLength(
                self.get().as_ptr(),
                length.as_secs_f64(),
                true,
            );
        }
    }
    pub fn set_selected(&self, selected: bool) {
        unsafe {
            Reaper::get()
                .low()
                .SetMediaItemSelected(self.get().as_ptr(), selected)
        }
    }
    pub fn delete(self) {
        unsafe {
            Reaper::get().low().DeleteTrackMediaItem(
                self.track().get().as_ptr(),
                self.get().as_ptr(),
            );
        }
    }

    pub fn make_only_selected_item(&mut self) {
        let mut pr = Project::new(self.project().context());
        pr.select_all_items(false);
        self.set_selected(true)
    }

    pub fn split(
        self,
        position: impl Into<Position>,
    ) -> ReaperStaticResult<ItemSplit<'a>> {
        let position: f64 = position.into().into();
        let ptr = unsafe {
            Reaper::get()
                .low()
                .SplitMediaItem(self.get().as_ptr(), position)
        };
        let ptr = match MediaItem::new(ptr) {
            None => {
                return Err(ReaperError::InvalidObject(
                    "Can not split item, probably, bad position.",
                ))
            }
            Some(ptr) => ptr,
        };
        Ok(ItemSplit {
            left_ptr: self.get(),
            right_ptr: ptr,
            project: self.project,
        })
    }

    pub fn update(&mut self) {
        unsafe { Reaper::get().low().UpdateItemInProject(self.get().as_ptr()) }
    }

    pub fn move_to_track(&self, track_index: usize) -> ReaperStaticResult<()> {
        let track =
            Track::<Immutable>::from_index(self.project(), track_index)
                .ok_or(ReaperError::InvalidObject(
                    "No track with given index!",
                ))?;
        let track_ptr = track.get().as_ptr();
        let result = unsafe {
            Reaper::get()
                .low()
                .MoveMediaItemToTrack(self.get().as_ptr(), track_ptr)
        };
        match result {
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not move item."))
            }
            true => Ok(()),
        }
    }

    fn set_info_value(
        &mut self,
        category: impl Into<String>,
        value: f64,
    ) -> ReaperStaticResult<()> {
        let mut category = category.into();
        let result = unsafe {
            Reaper::get().low().SetMediaItemInfo_Value(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                value,
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not set value."))
            }
        }
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.set_info_value("B_MUTE", muted as i32 as f64).unwrap()
    }
    /// muted (ignores solo). setting this value will not affect
    /// [Item::solo_override].
    pub fn set_mute_actual(&mut self, mute: bool) {
        self.set_info_value("B_MUTE_ACTUAL", mute as i32 as f64)
            .unwrap()
    }
    /// Basically, a way to solo particular item.
    ///
    /// This function will not override the same parameters on other items
    pub fn set_solo_override(&mut self, solo: ItemSoloOverride) {
        self.set_info_value("C_MUTE_SOLO", solo.int_value() as f64)
            .unwrap()
    }
    pub fn set_looped(&mut self, looped: bool) {
        self.set_info_value("B_LOOPSRC", looped as i32 as f64)
            .unwrap()
    }
    pub fn set_all_takes_play(&mut self, should_play: bool) {
        self.set_info_value("B_ALLTAKESPLAY", should_play as i32 as f64)
            .unwrap()
    }
    pub fn set_time_base(&mut self, mode: TimeMode) {
        self.set_info_value("C_BEATATTACHMODE", mode.int_value() as f64)
            .unwrap()
    }
    pub fn set_auto_stretch(&mut self, state: bool) {
        self.set_info_value("C_AUTOSTRETCH", state as i32 as f64)
            .unwrap()
    }
    pub fn set_locked(&mut self, locked: bool) {
        self.set_info_value("C_LOCK", locked as i32 as f64).unwrap()
    }
    pub fn set_volume(&mut self, volume: Volume) {
        self.set_info_value("D_VOL", volume.get()).unwrap()
    }
    pub fn set_snap_offset(
        &mut self,
        offset: Duration,
    ) -> ReaperStaticResult<()> {
        self.set_info_value("D_SNAPOFFSET", offset.as_secs_f64())
    }
    pub fn set_fade_in(&mut self, fade: ItemFade) -> ReaperStaticResult<()> {
        self.set_info_value("D_FADEINLEN", fade.length.as_secs_f64())?;
        self.set_info_value("D_FADEINDIR", fade.curve)?;
        self.set_info_value("C_FADEINSHAPE", fade.shape.int_value() as f64)?;
        self.set_info_value(
            "D_FADEINLEN_AUTO",
            fade.auto_fade_length as i32 as f64,
        )?;
        Ok(())
    }
    pub fn set_fade_out(&mut self, fade: ItemFade) -> ReaperStaticResult<()> {
        self.set_info_value("D_FADEOUTLEN", fade.length.as_secs_f64())?;
        self.set_info_value("D_FADEOUTDIR", fade.curve)?;
        self.set_info_value("C_FADEOUTSHAPE", fade.shape.int_value() as f64)?;
        self.set_info_value(
            "D_FADEOUTLEN_AUTO",
            fade.auto_fade_length as i32 as f64,
        )?;
        Ok(())
    }
    pub fn set_group_id(&mut self, id: usize) -> ReaperStaticResult<()> {
        self.set_info_value("I_GROUPID", id as f64)
    }
    /// Y-position (relative to top of track) in pixels when track is in free
    /// mode
    ///
    /// 0 → top of track, 1 → bottom of track (will never be 1)
    pub fn set_y_pos_free_mode(
        &mut self,
        y_pos: f64,
    ) -> ReaperStaticResult<()> {
        assert!((0.0..1.0).contains(&y_pos));
        self.set_info_value("F_FREEMODE_Y", y_pos as f64)
    }
    /// Height in pixels, when track is in free mode
    ///
    /// 0 → no height (will never be), 1 → full track.
    pub fn set_height_free_mode(&mut self, height: f64) {
        assert!((0.0..1.0).contains(&height));
        self.set_info_value("F_FREEMODE_H", height).unwrap()
    }
    /// if None → default.
    pub fn set_color(&mut self, color: Option<Color>) {
        let color = match color {
            None => 0,
            Some(color) => color.to_native() | 0x1000000,
        };
        self.set_info_value("I_CUSTOMCOLOR", color as f64).unwrap()
    }

    fn set_info_string(
        &self,
        category: impl Into<String>,
        value: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        let mut category = category.into();
        let value = value.into();
        let buf = as_c_string(&value).into_raw();
        let result = unsafe {
            Reaper::get().low().GetSetMediaItemInfo_String(
                self.get().as_ptr(),
                as_c_str(category.with_null()).as_ptr(),
                buf,
                true,
            )
        };
        match result {
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not get value!"))
            }
            true => Ok(()),
        }
    }
    pub fn set_notes(
        &self,
        notes: impl Into<String>,
    ) -> ReaperStaticResult<()> {
        let notes: String = notes.into();
        self.set_info_string("P_NOTES", notes)
    }
    pub fn set_guid(&self, guid: GUID) -> ReaperStaticResult<()> {
        let guid_str = guid.to_string();
        self.set_info_string("GUID", guid_str)
    }
}

/// Holds two new items after [Item::split].
#[derive(Debug)]
pub struct ItemSplit<'a> {
    left_ptr: MediaItem,
    right_ptr: MediaItem,
    project: &'a Project,
}
impl<'a> ItemSplit<'a> {
    fn left(&mut self) -> Item<'a, Mutable> {
        Item::new(self.project, self.left_ptr)
    }
    fn right(&mut self) -> Item<'a, Mutable> {
        Item::new(self.project, self.right_ptr)
    }
    pub fn get(mut self) -> (Item<'a, Mutable>, Item<'a, Mutable>) {
        (self.left(), self.right())
    }
}

/// Item personal solo state.
///
/// If soloed → other items and tracks will be not
/// overrided
#[repr(i32)]
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, IntEnum, Serialize, Deserialize,
)]
pub enum ItemSoloOverride {
    Soloed = -1,
    NoOverride = 0,
    NotSoloed = 1,
}

/// Item FadeIn\FadeOut parameters.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ItemFade {
    pub length: Duration,
    pub curve: f64,
    pub shape: ItemFadeShape,
    /// if true — length is not counted.
    pub auto_fade_length: bool,
}
impl ItemFade {
    pub fn new(
        length: Duration,
        curve: f64,
        shape: ItemFadeShape,
        auto_fade_length: bool,
    ) -> Self {
        Self {
            length,
            curve,
            shape,
            auto_fade_length,
        }
    }
}

/// Item fade-in\fade-out shape.
///
/// # Note
///
/// Shape affects curve attribute of [ItemFade]
#[repr(i32)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntEnum, Serialize, Deserialize,
)]
pub enum ItemFadeShape {
    Linear = 0,
    EqualPower = 1,
    SlightInner = 2,
    FastEnd = 3,
    FastStart = 4,
    SlowStartEnd = 5,
    Beizer = 6,
}
