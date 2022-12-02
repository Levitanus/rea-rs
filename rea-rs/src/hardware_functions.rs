use std::mem::MaybeUninit;

use log::debug;

use crate::{
    utils::{as_string, make_string_buf},
    HardwareSocket, Reaper, SampleAmount,
};

impl Reaper {
    /// Get latency in samples.
    ///
    /// Returns `(input, output)` latency.
    pub fn get_latency(&self) -> (SampleAmount, SampleAmount) {
        unsafe {
            let (mut input, mut output) =
                (MaybeUninit::new(0), MaybeUninit::new(0));
            self.low().GetInputOutputLatency(
                input.as_mut_ptr(),
                output.as_mut_ptr(),
            );
            (
                SampleAmount::new(input.assume_init() as u32),
                SampleAmount::new(output.assume_init() as u32),
            )
        }
    }

    /// Try to evaluate samplerate from the latency parameters.
    ///
    /// Not stable, and can be not precise.
    pub fn get_approximate_samplerate(&self) -> u32 {
        let secs_raw = self.low().GetOutputLatency();
        let (_, samples) = self.get_latency();
        debug!(
            "latency in samples: {:?}, latency in seconds: {:?}",
            samples, secs_raw
        );
        let rate = samples.get() as f64 / secs_raw;
        rate as u32
    }

    /// Open all audio and MIDI devices (if not opened).
    pub fn audio_init(&self) {
        self.low().Audio_Init()
    }

    /// Reset all MIDI devices.
    pub fn midi_reinit(&self) {
        self.low().midi_reinit()
    }

    /// Return whether audio is in pre-buffer (thread safe).
    pub fn audio_is_pre_buffer(&self) -> bool {
        self.low().Audio_IsPreBuffer() != 0
    }

    /// Return whether audio is running (thread safe).
    pub fn audio_is_running(&self) -> bool {
        self.low().Audio_IsRunning() != 0
    }

    pub fn get_n_audio_inputs(&self) -> usize {
        self.low().GetNumAudioInputs() as usize
    }

    pub fn get_n_audio_outputs(&self) -> usize {
        self.low().GetNumAudioOutputs() as usize
    }

    pub fn iter_audio_inputs(&self) -> AudioInputsIterator {
        AudioInputsIterator::new(*self.low(), self.get_n_audio_inputs())
    }

    pub fn iter_audio_outputs(&self) -> AudioOutputsIterator {
        AudioOutputsIterator::new(*self.low(), self.get_n_audio_outputs())
    }

    pub fn get_max_midi_inputs(&self) -> usize {
        self.low().GetMaxMidiInputs() as usize
    }

    pub fn get_max_midi_outputs(&self) -> usize {
        self.low().GetMaxMidiOutputs() as usize
    }

    pub fn get_n_midi_inputs(&self) -> usize {
        self.low().GetNumMIDIInputs() as usize
    }

    pub fn get_n_midi_outputs(&self) -> usize {
        self.low().GetNumMIDIOutputs() as usize
    }

    pub fn iter_midi_inputs(&self) -> MidiInputsIterator {
        MidiInputsIterator::new(*self.low(), self.get_n_midi_inputs())
    }

    pub fn iter_midi_outputs(&self) -> MidiOutputsIterator {
        MidiOutputsIterator::new(*self.low(), self.get_n_midi_outputs())
    }
}

pub struct AudioInputsIterator {
    low: reaper_low::Reaper,
    index: usize,
    amount: usize,
}
impl AudioInputsIterator {
    pub fn new(reaper: reaper_low::Reaper, num_inputs: usize) -> Self {
        Self {
            low: reaper,
            index: 0,
            amount: num_inputs,
        }
    }
}
impl Iterator for AudioInputsIterator {
    type Item = HardwareSocket;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.amount {
            return None;
        }
        let name = self.low.GetInputChannelName(self.index as i32);
        let name = as_string(name).expect("Can not get name for the channel");
        self.index += 1;
        Some(HardwareSocket::new(self.index as u32 - 1, name))
    }
}

pub struct AudioOutputsIterator {
    low: reaper_low::Reaper,
    index: usize,
    amount: usize,
}
impl AudioOutputsIterator {
    pub fn new(reaper: reaper_low::Reaper, num_outputs: usize) -> Self {
        Self {
            low: reaper,
            index: 0,
            amount: num_outputs,
        }
    }
}
impl Iterator for AudioOutputsIterator {
    type Item = HardwareSocket;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.amount {
            return None;
        }
        let name = self.low.GetOutputChannelName(self.index as i32);
        let name = as_string(name).expect("Can not get name for the channel");
        self.index += 1;
        Some(HardwareSocket::new(self.index as u32 - 1, name))
    }
}

pub struct MidiInputsIterator {
    low: reaper_low::Reaper,
    index: usize,
    amount: usize,
}
impl MidiInputsIterator {
    pub fn new(reaper: reaper_low::Reaper, num_inputs: usize) -> Self {
        Self {
            low: reaper,
            index: 0,
            amount: num_inputs,
        }
    }
}
impl Iterator for MidiInputsIterator {
    type Item = HardwareSocket;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.amount {
            return None;
        }
        let size = 1024;
        let buf = make_string_buf(size as usize);
        unsafe {
            let status =
                self.low.GetMIDIInputName(self.index as i32, buf, size);
            if status == false {
                return None;
            }
            let name =
                as_string(buf).expect("Can not get name for the channel");
            self.index += 1;
            Some(HardwareSocket::new(self.index as u32 - 1, name))
        }
    }
}

pub struct MidiOutputsIterator {
    low: reaper_low::Reaper,
    index: usize,
    amount: usize,
}
impl MidiOutputsIterator {
    pub fn new(reaper: reaper_low::Reaper, num_outputs: usize) -> Self {
        Self {
            low: reaper,
            index: 0,
            amount: num_outputs,
        }
    }
}
impl Iterator for MidiOutputsIterator {
    type Item = HardwareSocket;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.amount {
            return None;
        }
        let size = 1024;
        let buf = make_string_buf(size as usize);
        unsafe {
            let status =
                self.low.GetMIDIOutputName(self.index as i32, buf, size);
            if status == false {
                return None;
            }
            let name =
                as_string(buf).expect("Can not get name for the channel");
            self.index += 1;
            Some(HardwareSocket::new(self.index as u32 - 1, name))
        }
    }
}
