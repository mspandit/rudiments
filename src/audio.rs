use rodio::{OutputStream, dynamic_mixer, source::Source};
use std::{collections::HashMap, fmt, path::Path, thread, time::Duration};

use crate::{
    error::{Error::*, Result},
    instrumentation::{SampleFile, SampleSource},
    pattern::{Amplitude, Steps},
};

/// Number of playback channels.
const CHANNELS: u16 = 1;

/// Sample rate of playback.
const SAMPLE_RATE: u32 = 44_100;

/// Represents the playback tempo (beats per minute).
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct Tempo(u16);

impl From<u16> for Tempo {
    #[inline]
    fn from(v: u16) -> Tempo {
        Tempo(v)
    }
}

impl fmt::Display for Tempo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Tempo {
    /// Computes the duration of a step.
    pub fn step_duration(&self, beats: usize) -> Duration {
        Duration::from_secs_f32((beats as f32) * 15.0 / (self.0 as f32))
    }

    /// Computes the duration to delay a mix with trailing silence when played on repeat.
    /// This is necessary so that playback of the next iteration begins at the end
    /// of the current iteration's measure instead of after its final non-silent step.
    fn delay_pad_duration(&self) -> Duration {
        self.step_duration(1).mul_f32(self.delay_factor()) * 1 as u32
    }

    /// Computes a factor necessary for delay-padding a mix played on repeat.
    fn delay_factor(&self) -> f32 {
        -1.0 / 120.0 * self.0 as f32 + 2.0
    }
}

pub struct Sources(HashMap<SampleSource, (Steps, Amplitude)>);

impl Sources {
    /// Mixes the sources together using audio files found in the path given.
    pub fn mix(
        &self,
        tempo: &Tempo,
    ) -> Result<Box<dyn Source<Item = i16> + Send>> {
        let (controller, mixer) = dynamic_mixer::mixer(CHANNELS, SAMPLE_RATE);
        for (sample_source, (steps, amplitude)) in self.0.iter() {
            for (i, step) in steps.iter().enumerate() {
                if !step {
                    continue;
                }
                let delay = tempo.step_duration(1) * (i as u32);
                controller.add(sample_source.source.clone().amplify(amplitude.value()).delay(delay));
            }
        }
        Ok(Box::new(mixer))
    }
}

/// A type that represents the fully bound and reduced tracks of a pattern.
pub struct Tracks(HashMap<SampleFile, (Steps, Amplitude)>);

impl Tracks {
    /// Creates sources using audio files found in the path given.
    pub fn sources(&self, samples_path: &Path) -> Result<Sources> {
        let mut sample_map = HashMap::new();
        for (sample_file, (steps, amplitude)) in self.0.iter() {
            sample_map.insert(
                SampleSource::from(samples_path, sample_file)?,
                (steps.clone(), amplitude.clone())
            );
        }
        Ok(Sources(sample_map))
    }

    pub fn from(hash_map: HashMap<SampleFile, (Steps, Amplitude)>) -> Tracks {
        Tracks(hash_map)
    }
}

/// Plays a mixed pattern repeatedly.
pub fn play_repeat(
    tempo: &Tempo,
    source: Box<dyn Source<Item = i16> + Send>,
    beats: usize,
) -> Result<()> {
    if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
        // play the pattern
        if let Ok(()) = stream_handle.play_raw(
            source
                // forward pad with trailing silence
                .delay(tempo.delay_pad_duration())
                // trim to measure length
                .take_duration(tempo.step_duration(beats))
                .repeat_infinite()
                .convert_samples(),
        ) {
            // sleep forever
            thread::park();
            Ok(())
        } else {
            Err(AudioDeviceError())
        }
    } else {
        Err(AudioDeviceError())
    }
}

/// Plays a mixed pattern once.
pub fn play_once(
    tempo: &Tempo,
    source: Box<dyn Source<Item = i16> + Send>,
    beats: usize
) -> Result<()> {
    if let Ok((_stream, stream_handle)) = OutputStream::try_default() {
        // play the pattern
        if let Ok(_) = stream_handle.play_raw(source.convert_samples()) {
            // sleep for the duration of a single measure
            thread::sleep(tempo.step_duration(beats));
            Ok(())
        } else {
            Err(AudioDeviceError())
        }
    } else {
        Err(AudioDeviceError())
    }
}
