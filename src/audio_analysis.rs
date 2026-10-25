use rodio::{ChannelCount, Sample, SampleRate, Source, source::SeekError};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};
use std::time::Duration;

const LOW_PASS_CUTOFF_HZ: f32 = 180.0;
const ANALYSIS_UPDATES_PER_SECOND: f32 = 60.0;
const MIN_LEVEL_DB: f32 = -55.0;
const SILENCE_EPSILON: f32 = 1e-7;
const MIN_REFERENCE_LEVEL: f32 = 0.12;
const BASELINE_TIME_CONSTANT_SECONDS: f32 = 8.0;
const RELATIVE_LEVEL_CENTER: f32 = 0.45;
const RELATIVE_LEVEL_WIDTH: f32 = 1.25;
const FAST_ATTACK_SECONDS: f32 = 0.012;
const FAST_RELEASE_SECONDS: f32 = 0.16;
const SLOW_ATTACK_SECONDS: f32 = 0.08;
const SLOW_RELEASE_SECONDS: f32 = 0.5;

#[derive(Debug, Clone, Copy, Default)]
pub struct AudioFeatures {
    pub bass_slow: f32,
    pub bass_pulse: f32,
}

/// Lock-free audio features shared between the audio source and the UI.
#[derive(Clone, Default)]
pub struct BassAnalyzer {
    slow_level: Arc<AtomicU32>,
    pulse_level: Arc<AtomicU32>,
}

impl BassAnalyzer {
    pub fn features(&self) -> AudioFeatures {
        AudioFeatures {
            bass_slow: f32::from_bits(self.slow_level.load(Ordering::Relaxed)),
            bass_pulse: f32::from_bits(self.pulse_level.load(Ordering::Relaxed)),
        }
    }

    pub fn reset(&self) {
        self.slow_level.store(0.0_f32.to_bits(), Ordering::Relaxed);
        self.pulse_level.store(0.0_f32.to_bits(), Ordering::Relaxed);
    }

    pub fn tap<S>(&self, source: S) -> BassTapSource<S>
    where
        S: Source,
    {
        BassTapSource::new(source, self.clone())
    }

    fn publish(&self, features: AudioFeatures) {
        self.slow_level.store(
            features.bass_slow.clamp(0.0, 1.0).to_bits(),
            Ordering::Relaxed,
        );
        self.pulse_level.store(
            features.bass_pulse.clamp(0.0, 1.0).to_bits(),
            Ordering::Relaxed,
        );
    }
}

/// A rodio source wrapper that measures low-frequency energy while audio is decoded.
pub struct BassTapSource<S> {
    inner: S,
    analyzer: BassAnalyzer,
    channels: usize,
    channel_index: usize,
    frame_sum: f32,
    low_pass_alpha: f32,
    low_pass_state: f32,
    energy_sum: f32,
    energy_samples: usize,
    energy_window: usize,
    baseline_level: f32,
    fast_envelope: f32,
    slow_envelope: f32,
}

impl<S> BassTapSource<S>
where
    S: Source,
{
    fn new(inner: S, analyzer: BassAnalyzer) -> Self {
        let sample_rate = inner.sample_rate().get() as f32;
        let channels = inner.channels().get() as usize;
        let energy_window = (sample_rate / ANALYSIS_UPDATES_PER_SECOND).max(128.0) as usize;

        Self {
            inner,
            analyzer,
            channels,
            channel_index: 0,
            frame_sum: 0.0,
            low_pass_alpha: 1.0 - (-std::f32::consts::TAU * LOW_PASS_CUTOFF_HZ / sample_rate).exp(),
            low_pass_state: 0.0,
            energy_sum: 0.0,
            energy_samples: 0,
            energy_window,
            baseline_level: 0.0,
            fast_envelope: 0.0,
            slow_envelope: 0.0,
        }
    }

    fn process_frame(&mut self) {
        let mono = self.frame_sum / self.channels as f32;
        self.frame_sum = 0.0;

        self.low_pass_state += self.low_pass_alpha * (mono - self.low_pass_state);
        self.energy_sum += self.low_pass_state * self.low_pass_state;
        self.energy_samples += 1;

        if self.energy_samples >= self.energy_window {
            let rms = (self.energy_sum / self.energy_samples as f32).sqrt();
            self.update_features(normalize_low_frequency_rms(rms));

            self.energy_sum = 0.0;
            self.energy_samples = 0;
        }
    }

    fn update_features(&mut self, normalized_level: f32) {
        let delta_seconds = 1.0 / ANALYSIS_UPDATES_PER_SECOND;

        if self.baseline_level <= f32::EPSILON {
            self.baseline_level = normalized_level.max(MIN_REFERENCE_LEVEL);
        } else {
            let response = 1.0 - (-delta_seconds / BASELINE_TIME_CONSTANT_SECONDS).exp();
            self.baseline_level += (normalized_level - self.baseline_level) * response;
        }

        let relative_level =
            (normalized_level / self.baseline_level.max(MIN_REFERENCE_LEVEL)).clamp(0.0, 2.0);

        // Center the envelope around the recent average so sustained bass does
        // not pin the visual level at 1.0 for the entire song.
        let activation =
            ((relative_level - RELATIVE_LEVEL_CENTER) / RELATIVE_LEVEL_WIDTH).clamp(0.0, 1.0);

        self.fast_envelope += (activation - self.fast_envelope)
            * envelope_response(
                delta_seconds,
                FAST_ATTACK_SECONDS,
                FAST_RELEASE_SECONDS,
                activation > self.fast_envelope,
            );
        self.slow_envelope += (activation - self.slow_envelope)
            * envelope_response(
                delta_seconds,
                SLOW_ATTACK_SECONDS,
                SLOW_RELEASE_SECONDS,
                activation > self.slow_envelope,
            );

        let pulse = ((self.fast_envelope - self.slow_envelope) * 3.0).clamp(0.0, 1.0);

        self.analyzer.publish(AudioFeatures {
            bass_slow: self.slow_envelope,
            bass_pulse: pulse,
        });
    }

    fn reset_analysis(&mut self) {
        self.channel_index = 0;
        self.frame_sum = 0.0;
        self.low_pass_state = 0.0;
        self.energy_sum = 0.0;
        self.energy_samples = 0;
        self.baseline_level = 0.0;
        self.fast_envelope = 0.0;
        self.slow_envelope = 0.0;
        self.analyzer.reset();
    }
}

fn envelope_response(delta_seconds: f32, attack: f32, release: f32, rising: bool) -> f32 {
    let time_constant = if rising { attack } else { release };
    1.0 - (-delta_seconds / time_constant).exp()
}

impl<S> Iterator for BassTapSource<S>
where
    S: Source,
{
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;

        self.frame_sum += sample;
        self.channel_index += 1;
        if self.channel_index >= self.channels {
            self.channel_index = 0;
            self.process_frame();
        }

        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<S> Source for BassTapSource<S>
where
    S: Source,
{
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.inner.try_seek(pos)?;
        self.reset_analysis();
        Ok(())
    }
}

fn normalize_low_frequency_rms(rms: f32) -> f32 {
    let db = 20.0 * (rms + SILENCE_EPSILON).log10();
    ((db - MIN_LEVEL_DB) / -MIN_LEVEL_DB).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{BassAnalyzer, normalize_low_frequency_rms};
    use rodio::buffer::SamplesBuffer;
    use std::num::NonZero;

    #[test]
    fn normalized_level_is_bounded() {
        assert_eq!(normalize_low_frequency_rms(0.0), 0.0);
        assert_eq!(normalize_low_frequency_rms(1.0), 1.0);
        assert!(normalize_low_frequency_rms(0.01) > 0.0);
        assert!(normalize_low_frequency_rms(0.01) < 1.0);
    }

    #[test]
    fn tap_source_publishes_audio_features() {
        let sample_rate = NonZero::new(44_100).unwrap();
        let channels = NonZero::new(1).unwrap();
        let mut samples = vec![0.0_f32; 4_410];
        samples.extend(vec![0.25_f32; 4_410]);
        let source = SamplesBuffer::new(channels, sample_rate, samples);
        let analyzer = BassAnalyzer::default();
        let mut tap = analyzer.tap(source);
        let mut maximum_pulse = 0.0_f32;

        for _ in 0..8_820 {
            tap.next();
            maximum_pulse = maximum_pulse.max(analyzer.features().bass_pulse);
        }

        let features = analyzer.features();
        assert!(features.bass_slow > 0.1);
        assert!(features.bass_pulse < 1.0);
        assert!(maximum_pulse > 0.05);
    }
}
