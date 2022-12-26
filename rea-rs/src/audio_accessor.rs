use std::marker::PhantomData;

use crate::{
    errors::{ReaperError, ReaperResult},
    ptr_wrappers, KnowsProject, Mutable, Position, ProbablyMutable, Reaper,
    SampleAmount, WithReaperPtr,
};

#[derive(Debug, PartialEq)]
pub struct AudioAccessor<'a, T: KnowsProject, P: ProbablyMutable> {
    ptr: ptr_wrappers::AudioAccessor,
    parent: &'a T,
    should_check: bool,
    phantom: PhantomData<P>,
}
impl<'a, T: KnowsProject, P: ProbablyMutable> WithReaperPtr
    for AudioAccessor<'a, T, P>
{
    type Ptr = ptr_wrappers::AudioAccessor;
    fn get_pointer(&self) -> Self::Ptr {
        self.ptr
    }
    fn get(&self) -> Self::Ptr {
        self.require_valid_2(self.parent.project())
            .expect("Object no longer is valid.");
        self.ptr
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
impl<'a, T: KnowsProject, P: ProbablyMutable> AudioAccessor<'a, T, P> {
    pub fn new(parent: &'a T, ptr: ptr_wrappers::AudioAccessor) -> Self {
        Self {
            ptr,
            parent,
            should_check: true,
            phantom: PhantomData,
        }
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

    /// Get buffer of samples with given absolute project position in samples.
    ///
    /// Returned buffer is `Vec<f64>` of length samples_per_channel *
    /// n_channels: values placed sample by sample for each channel in a
    /// turn. E.g. [spl1_ch1, spl1_ch2, spl1_ch3, spl2_ch1, spl2_ch2,
    /// spl3_ch3]
    ///
    /// `start` is amount of samples since audio accessor start time.
    ///
    /// You can look at the routing scheme to see where AudioAccessor
    /// is connected in case of track or take parenting:
    /// <https://wiki.cockos.com/wiki/images/5/50/SWS_loudness_analysis_signal_flow_chart.png>
    ///
    /// Example from SWS:
    /// <https://github.com/reaper-oss/sws/blob/bcc8fbc96f30a943bd04fb8030b4a03ea1ff7557/Breeder/BR_Loudness.cpp#L1020-L1079>
    ///
    /// # note
    ///
    /// Samples converted back to seconds as persized as possible,
    /// but I still afraid a bit of this function being sample-accurate.
    pub fn get_sample_block_raw(
        &self,
        start: SampleAmount,
        samples_per_channel: u32,
        n_channels: u8,
        samplerate: u32,
    ) -> ReaperResult<Option<Vec<f64>>> {
        let mut sample_buffer =
            vec![0.0; (samples_per_channel * n_channels as u32) as usize];
        let start = start.as_time(samplerate) + self.start().as_duration();
        let result = unsafe {
            Reaper::get().low().GetAudioAccessorSamples(
                self.get().as_ptr(),
                samplerate as i32,
                n_channels as i32,
                start.as_secs_f64(),
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
}
impl<'a, T: KnowsProject> AudioAccessor<'a, T, Mutable> {
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
impl<'a, T: KnowsProject, P: ProbablyMutable> Drop
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
