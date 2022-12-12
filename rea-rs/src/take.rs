use std::{
    ffi::c_char,
    mem::{transmute, MaybeUninit},
};

use crate::{
    errors::{ReaperError, ReaperStaticResult},
    utils::{as_c_str, as_string, WithNull},
    AudioAccessor, Fx, Immutable, Item, KnowsProject, MidiEventBuilder,
    Mutable, ProbablyMutable, Project, Reaper, Source, TakeFX, WithReaperPtr,
};
use reaper_medium::{MediaItemTake, PcmSource};

#[derive(Debug, PartialEq)]
pub struct Take<'a, T: ProbablyMutable> {
    ptr: MediaItemTake,
    should_check: bool,
    item: &'a Item<'a, T>,
}
impl<'a, T: ProbablyMutable> KnowsProject for Take<'a, T> {
    fn project(&self) -> &Project {
        self.item.project()
    }
}
impl<'a, T: ProbablyMutable> WithReaperPtr<'a> for Take<'a, T> {
    type Ptr = MediaItemTake;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.item().project()).unwrap();
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
impl<'a, T: ProbablyMutable> Take<'a, T> {
    pub fn new(ptr: MediaItemTake, item: &'a Item<'a, T>) -> Self {
        Self {
            ptr,
            should_check: true,
            item,
        }
    }
    pub fn item(&self) -> &Item<T> {
        self.item
    }
    pub fn get_fx(&'a self, index: usize) -> Option<TakeFX<'a, T>> {
        TakeFX::from_index(self, index)
    }
    pub fn is_active(&self) -> bool {
        self.get() == self.item().active_take().get()
    }
    pub fn is_midi(&self) -> bool {
        unsafe { Reaper::get().low().TakeIsMIDI(self.get().as_ptr()) }
    }
    pub fn n_envelopes(&self) -> usize {
        unsafe {
            Reaper::get().low().CountTakeEnvelopes(self.get().as_ptr())
                as usize
        }
    }
    pub fn n_fx(&self) -> usize {
        unsafe {
            Reaper::get().low().TakeFX_GetCount(self.get().as_ptr()) as usize
        }
    }
    pub fn n_midi_events(&self) -> usize {
        let mut notes = MaybeUninit::zeroed();
        let mut cc = MaybeUninit::zeroed();
        let mut sysex = MaybeUninit::zeroed();
        unsafe {
            Reaper::get().low().MIDI_CountEvts(
                self.get().as_ptr(),
                notes.as_mut_ptr(),
                cc.as_mut_ptr(),
                sysex.as_mut_ptr(),
            ) as usize
        }
    }
    pub fn name(&self) -> String {
        let result =
            unsafe { Reaper::get().low().GetTakeName(self.get().as_ptr()) };
        as_string(result).expect("Can not convert name to string")
    }

    pub fn source(&self) -> Option<Source<Immutable>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetMediaItemTake_Source(self.get().as_ptr())
        };
        match PcmSource::new(ptr) {
            None => None,
            Some(ptr) => Some(Source::new(unsafe { transmute(self) }, ptr)),
        }
    }

    pub fn iter_midi(
        &self,
        buf_size_override: impl Into<Option<i32>>,
    ) -> ReaperStaticResult<MidiEventBuilder> {
        let buf = self.get_midi(buf_size_override)?;
        Ok(MidiEventBuilder::new(buf.into_iter()))
    }

    /// Get take raw midi data.
    ///
    /// It is quite useless as it is, but, it can be used several times with
    /// [MidiEventBuilder] for iterating through various event types.
    pub fn get_midi(
        &self,
        buf_size_override: impl Into<Option<i32>>,
    ) -> ReaperStaticResult<Vec<u8>> {
        let size = buf_size_override.into().unwrap_or(i32::MAX - 100);
        let mut buf = vec![0_u8; size as usize];
        let raw = buf.as_mut_ptr() as *mut c_char;
        let mut size = MaybeUninit::new(size);
        let result = unsafe {
            Reaper::get().low().MIDI_GetAllEvts(
                self.get().as_ptr(),
                raw,
                size.as_mut_ptr(),
            )
        };
        let size = unsafe { size.assume_init() };
        buf.truncate(size as usize);
        match result {
            false => {
                Err(ReaperError::UnsuccessfulOperation("Can not get midi"))
            }
            true => Ok(buf),
        }
    }
}

impl<'a> Take<'a, Mutable> {
    pub fn add_audio_accessor(
        &mut self,
    ) -> AudioAccessor<'a, Take<Mutable>, Mutable> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .CreateTakeAudioAccessor(self.get().as_ptr())
        };
        match reaper_medium::AudioAccessor::new(ptr) {
            None => panic!("Can not create audio accessor"),
            Some(ptr) => AudioAccessor::new(self, ptr),
        }
    }

    pub fn set_active(&mut self) {
        unsafe { Reaper::get().low().SetActiveTake(self.get().as_ptr()) }
    }

    /// Add FX at given position, or return existing one.
    ///
    /// If `even_if_exists` is `false`, plugin will be added only
    /// if no plugin exists on track.
    ///
    /// Otherwise, if position is None → the last slot will be used.
    /// The resulting FX will have real index, that may differ from the
    /// desired.
    pub fn add_fx(
        &mut self,
        name: impl Into<String>,
        position: impl Into<Option<u8>>,
        even_if_exists: bool,
    ) -> Option<TakeFX<Mutable>> {
        let insatantinate = match even_if_exists {
            false => 1 as i32,
            true => match position.into() {
                None => -1 as i32,
                Some(pos) => -1000 - pos as i32,
            },
        };
        let index = unsafe {
            Reaper::get().low().TakeFX_AddByName(
                self.get().as_ptr(),
                as_c_str(name.into().with_null()).as_ptr(),
                insatantinate,
            )
        };
        TakeFX::<Mutable>::from_index(self, index as usize)
    }

    pub fn select_all_midi_events(&mut self, select: bool) {
        assert!(self.is_midi());
        unsafe {
            Reaper::get()
                .low()
                .MIDI_SelectAll(self.get().as_ptr(), select)
        }
    }

    pub fn sort_midi(&mut self) {
        assert!(self.is_midi());
        unsafe { Reaper::get().low().MIDI_Sort(self.get().as_ptr()) }
    }

    pub fn source_mut(&mut self) -> Option<Source<Mutable>> {
        let ptr = unsafe {
            Reaper::get()
                .low()
                .GetMediaItemTake_Source(self.get().as_ptr())
        };
        match PcmSource::new(ptr) {
            None => None,
            Some(ptr) => Some(Source::new(self, ptr)),
        }
    }

    pub fn set_source<T: ProbablyMutable>(
        &mut self,
        source: Source<T>,
    ) -> ReaperStaticResult<()> {
        let result = unsafe {
            Reaper::get().low().SetMediaItemTake_Source(
                self.get().as_ptr(),
                source.get().as_ptr(),
            )
        };
        match result {
            true => Ok(()),
            false => {
                Err(ReaperError::UnsuccessfulOperation("can not set source"))
            }
        }
    }
}
