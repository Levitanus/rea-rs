use std::marker::PhantomData;

use reaper_medium::{self, ReaperPointer};

use crate::{
    errors::{ReaperError, ReaperResult},
    Mutable, Position, ProbablyMutable, Project, Reaper, WithReaperPtr,
};

pub trait AudioAccessorParent<'a>
where
    Self: WithReaperPtr,
{
    fn project(&'a self) -> &'a Project;
}

pub struct AudioAccessor<'a, T: AudioAccessorParent<'a>, P: ProbablyMutable> {
    ptr: reaper_medium::AudioAccessor,
    parent: &'a T,
    should_check: bool,
    phantom: PhantomData<P>,
}
impl<'a, T: AudioAccessorParent<'a>, P: ProbablyMutable> WithReaperPtr
    for AudioAccessor<'a, T, P>
{
    fn get_pointer(&self) -> ReaperPointer {
        ReaperPointer::AudioAccessor(self.ptr)
    }
    fn make_unchecked(&mut self) {
        self.should_check = false
    }
    fn make_checked(&mut self) {
        self.should_check = true
    }
    fn should_check(&self) -> bool {
        self.should_check
    }
}
impl<'a, T: AudioAccessorParent<'a>, P: ProbablyMutable>
    AudioAccessor<'a, T, P>
{
    pub fn new(parent: &'a T, ptr: reaper_medium::AudioAccessor) -> Self {
        Self {
            ptr,
            parent,
            should_check: true,
            phantom: PhantomData,
        }
    }
    pub fn get(&self) -> reaper_medium::AudioAccessor {
        self.require_valid_2(self.parent.project())
            .expect("Object no longer is valid.");
        self.ptr
    }
    pub fn has_state_changed(&self) -> bool {
        unsafe {
            Reaper::get()
                .low()
                .AudioAccessorStateChanged(self.get().as_ptr())
        }
    }
    pub fn start(&self) -> Position {
        unsafe {
            Reaper::get()
                .low()
                .GetAudioAccessorStartTime(self.get().as_ptr())
                .into()
        }
    }
    pub fn end(&self) -> Position {
        unsafe {
            Reaper::get()
                .low()
                .GetAudioAccessorEndTime(self.get().as_ptr())
                .into()
        }
    }

    /// https://wiki.cockos.com/wiki/images/5/50/SWS_loudness_analysis_signal_flow_chart.png
    pub fn get_sample_block_raw(
        &self,
        start: Position,
        samples_per_channel: u32,
        n_channels: u8,
        samplerate: u32,
    ) -> ReaperResult<Option<Vec<f64>>> {
        let mut sample_buffer =
            vec![0.0; (samples_per_channel * n_channels as u32) as usize];
        let result = unsafe {
            Reaper::get().low().GetAudioAccessorSamples(
                self.get().as_ptr(),
                samplerate as i32,
                n_channels as i32,
                start.into(),
                samples_per_channel as i32,
                sample_buffer.as_mut_ptr(),
            )
        };
        match result {
            -1 => {
                Err(ReaperError::UnsuccessfulOperation("Can not get samples.")
                    .into())
            }
            0 => Ok(None),
            _ => Ok(Some(sample_buffer)),
        }
    }

    // pub fn iter_samples()
}
impl<'a, T: AudioAccessorParent<'a>> AudioAccessor<'a, T, Mutable> {
    /// Validates the current state of the audio accessor
    ///
    /// -- must ONLY call this from the main thread.
    ///
    /// Returns true if the state changed.
    pub fn validate(&mut self) -> bool {
        unsafe {
            Reaper::get()
                .low()
                .AudioAccessorValidateState(self.get().as_ptr())
        }
    }

    /// Force the accessor to reload its state from the underlying track or
    /// media item take.
    pub fn update(&mut self) {
        unsafe { Reaper::get().low().AudioAccessorUpdate(self.get().as_ptr()) }
    }
}
impl<'a, T: AudioAccessorParent<'a>, P: ProbablyMutable> Drop
    for AudioAccessor<'a, T, P>
{
    fn drop(&mut self) {
        unsafe {
            Reaper::get()
                .low()
                .DestroyAudioAccessor(self.get().as_ptr())
        }
    }
}
