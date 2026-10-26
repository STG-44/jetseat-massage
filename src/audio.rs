use crate::signal::{self, Pattern};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    f64::consts::TAU,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::Instant,
};

pub struct Endpoint {
    pub name: String,
    pub device: cpal::Device,
}

pub fn endpoints() -> Result<Vec<Endpoint>, String> {
    cpal::default_host()
        .output_devices()
        .map_err(|e| e.to_string())?
        .map(|device| {
            device
                .name()
                .map(|name| Endpoint { name, device })
                .map_err(|e| e.to_string())
        })
        .collect()
}

struct Shared {
    origin: Instant,
    active: AtomicBool,
    started_ms: AtomicU64,
    duration_ms: AtomicU64,
    heartbeat_ms: AtomicU64,
    pattern: AtomicU32,
    strength: AtomicU32,
    speed: AtomicU32,
    frequency: AtomicU32,
    meter: AtomicU32,
}

impl Shared {
    fn millis(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }

    fn elapsed(&self) -> f64 {
        self.millis()
            .saturating_sub(self.started_ms.load(Ordering::Relaxed)) as f64
            / 1000.0
    }

    fn is_live(&self) -> bool {
        self.active.load(Ordering::Acquire)
            && self
                .millis()
                .saturating_sub(self.heartbeat_ms.load(Ordering::Relaxed))
                < 1500
            && self.elapsed() < self.duration_ms.load(Ordering::Relaxed) as f64 / 1000.0
    }
}

pub struct Audio {
    shared: Arc<Shared>,
    stream: Option<cpal::Stream>,
    errors: Receiver<String>,
    error_tx: Sender<String>,
}

impl Default for Audio {
    fn default() -> Self {
        let (error_tx, errors) = mpsc::channel();
        Self {
            shared: Arc::new(Shared {
                origin: Instant::now(),
                active: AtomicBool::new(false),
                started_ms: AtomicU64::new(0),
                duration_ms: AtomicU64::new(0),
                heartbeat_ms: AtomicU64::new(0),
                pattern: AtomicU32::new(0),
                strength: AtomicU32::new(25),
                speed: AtomicU32::new(100),
                frequency: AtomicU32::new(70),
                meter: AtomicU32::new(0),
            }),
            stream: None,
            errors,
            error_tx,
        }
    }
}

impl Audio {
    pub fn configure(&self, pattern: Pattern, strength: u32, speed: f32, frequency: u32) {
        self.shared.pattern.store(pattern as u32, Ordering::Relaxed);
        self.shared
            .strength
            .store(strength.min(100), Ordering::Relaxed);
        self.shared
            .speed
            .store((speed.clamp(0.5, 2.0) * 100.0) as u32, Ordering::Relaxed);
        self.shared
            .frequency
            .store(frequency.clamp(40, 120), Ordering::Relaxed);
    }

    pub fn heartbeat(&self) {
        self.shared
            .heartbeat_ms
            .store(self.shared.millis(), Ordering::Relaxed);
    }

    pub fn start(&mut self, device: &cpal::Device, seconds: u64) -> Result<(), String> {
        self.stop();
        while self.errors.try_recv().is_ok() {}
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let tx = self.error_tx.clone();
        let shared = self.shared.clone();
        let stream_config = config.config();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => build::<f32>(device, &stream_config, shared, tx),
            cpal::SampleFormat::I16 => build::<i16>(device, &stream_config, shared, tx),
            cpal::SampleFormat::U16 => build::<u16>(device, &stream_config, shared, tx),
            cpal::SampleFormat::I32 => build::<i32>(device, &stream_config, shared, tx),
            cpal::SampleFormat::U32 => build::<u32>(device, &stream_config, shared, tx),
            cpal::SampleFormat::F64 => build::<f64>(device, &stream_config, shared, tx),
            format => return Err(format!("Аудиоформат {format:?} пока не поддерживается")),
        }?;
        self.shared
            .started_ms
            .store(self.shared.millis(), Ordering::Relaxed);
        self.shared
            .duration_ms
            .store(seconds.clamp(1, 1800) * 1000, Ordering::Relaxed);
        self.heartbeat();
        self.shared.active.store(true, Ordering::Release);
        if let Err(e) = stream.play() {
            self.shared.active.store(false, Ordering::Release);
            return Err(e.to_string());
        }
        self.stream = Some(stream);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.shared.active.store(false, Ordering::Release);
        self.shared.meter.store(0, Ordering::Relaxed);
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
        }
    }

    pub fn poll_error(&self) -> Option<String> {
        self.errors.try_recv().ok()
    }

    pub fn is_active(&self) -> bool {
        self.shared.active.load(Ordering::Acquire)
    }

    pub fn meter(&self) -> f32 {
        self.shared.meter.load(Ordering::Relaxed) as f32 / 1000.0
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.stop();
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    shared: Arc<Shared>,
    errors: Sender<String>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let sample_rate = config.sample_rate.0 as f64;
    let channels = config.channels as usize;
    let mut phase = 0.0;
    let mut smoothed_level = 0.0_f32;
    let mut smoothed_frequency = 70.0_f64;
    let smoothing = (1.0 - (-1.0 / (sample_rate * 0.02)).exp()) as f32;
    let error_shared = shared.clone();
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if !shared.is_live() {
                    // A missing GUI heartbeat or timer expiry latches off; playback
                    // cannot resume by itself when the GUI starts responding again.
                    shared.active.store(false, Ordering::Release);
                    shared.meter.store(0, Ordering::Relaxed);
                    smoothed_level = 0.0;
                    data.fill(T::from_sample(0.0));
                    return;
                }
                let t = shared.elapsed();
                let duration = shared.duration_ms.load(Ordering::Relaxed) as f64 / 1000.0;
                let pattern = Pattern::from_index(shared.pattern.load(Ordering::Relaxed));
                let strength = shared.strength.load(Ordering::Relaxed) as f32 / 100.0;
                let speed = shared.speed.load(Ordering::Relaxed) as f64 / 100.0;
                let frequency = shared.frequency.load(Ordering::Relaxed) as f64;
                for (i, frame) in data.chunks_mut(channels).enumerate() {
                    if !shared.active.load(Ordering::Acquire) {
                        frame.fill(T::from_sample(0.0));
                        continue;
                    }
                    let frame_t = t + i as f64 / sample_rate;
                    let target = signal::level(pattern, frame_t, speed, strength, duration);
                    smoothed_level += (target - smoothed_level) * smoothing;
                    smoothed_frequency += (frequency - smoothed_frequency) * smoothing as f64;
                    let value = if frame_t >= duration {
                        0.0
                    } else {
                        signal::sample(phase, smoothed_level)
                    };
                    frame.fill(T::from_sample(value));
                    phase = (phase + TAU * smoothed_frequency / sample_rate) % TAU;
                }
                shared
                    .meter
                    .store((smoothed_level * 1000.0) as u32, Ordering::Relaxed);
            },
            move |error| {
                error_shared.active.store(false, Ordering::Release);
                let _ = errors.send(error.to_string());
            },
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_timer_and_stop_gate_audio_independently_of_gui() {
        let audio = Audio::default();
        let shared = &audio.shared;
        shared.started_ms.store(shared.millis(), Ordering::Relaxed);
        shared.duration_ms.store(60_000, Ordering::Relaxed);
        audio.heartbeat();
        shared.active.store(true, Ordering::Release);
        assert!(shared.is_live());
        shared.duration_ms.store(0, Ordering::Relaxed);
        assert!(!shared.is_live());
        shared.duration_ms.store(60_000, Ordering::Relaxed);
        shared.active.store(false, Ordering::Release);
        assert!(!shared.is_live());
    }

    #[test]
    fn stale_gui_heartbeat_disables_output() {
        let mut audio = Audio::default();
        let shared = Arc::get_mut(&mut audio.shared).unwrap();
        shared.origin -= std::time::Duration::from_secs(2);
        shared.started_ms.store(shared.millis(), Ordering::Relaxed);
        shared.duration_ms.store(60_000, Ordering::Relaxed);
        shared.active.store(true, Ordering::Release);
        assert!(!shared.is_live());
        audio.heartbeat();
        assert!(audio.shared.is_live());
    }

    #[test]
    #[ignore = "opens the system audio output with zero signal; run explicitly"]
    fn real_audio_stream_silence_stop_and_watchdog() {
        let device = cpal::default_host()
            .default_output_device()
            .expect("No output device");
        let mut audio = Audio::default();
        audio.configure(Pattern::Steady, 0, 1.0, 70);
        audio.start(&device, 10).expect("Cannot start audio output");
        for _ in 0..20 {
            audio.heartbeat();
            std::thread::sleep(std::time::Duration::from_millis(25));
            assert!(audio.is_active(), "Audio stopped unexpectedly");
            assert_eq!(audio.meter(), 0.0);
            assert!(audio.poll_error().is_none());
        }
        std::thread::sleep(std::time::Duration::from_millis(1800));
        assert!(!audio.is_active(), "Watchdog did not silence the stream");
        audio.stop();
        assert!(audio.stream.is_none());
        audio.start(&device, 10).expect("Cannot restart after Stop");
        audio.stop();
        assert!(!audio.is_active());
    }
}
