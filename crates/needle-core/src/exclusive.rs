use crate::audio::PlaybackState;
use anyhow::{Context, Result};
use rodio::{Sink, Source, queue::SourcesQueueOutput};
use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};
use wasapi::*;

pub struct Output {
    stop: Arc<AtomicBool>,
    pub ready: Arc<AtomicBool>,
    pub failed: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Output {
    pub fn start(
        source: SourcesQueueOutput,
        sink: Weak<Sink>,
        device: Option<String>,
        state: Arc<Mutex<PlaybackState>>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let ready_worker = ready.clone();
        let failed = Arc::new(AtomicBool::new(false));
        let stop_worker = stop.clone();
        let failed_worker = failed.clone();
        let thread = std::thread::spawn(move || {
            while !ready_worker.load(Ordering::Acquire) {
                if stop_worker.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            if let Err(error) = render(source, sink, device, state.clone(), stop_worker) {
                failed_worker.store(true, Ordering::Relaxed);
                if let Ok(mut state) = state.lock() {
                    state.error = Some(format!(
                        "Exclusive output: {error:#}. Choose shared output or a supported device sample rate."
                    ));
                    state.playing = false;
                }
            }
        });
        Self {
            stop,
            ready,
            failed,
            thread: Some(thread),
        }
    }
}
impl Drop for Output {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Endpoint {
    client: AudioClient,
    renderer: AudioRenderClient,
    event: Handle,
    frames: usize,
    bytes: usize,
    channels: u16,
    rate: u32,
    started: bool,
}
impl Endpoint {
    fn open(device: &Device, rate: u32, channels: u16) -> Result<Self> {
        let mut client = device.get_iaudioclient()?;
        let format = native_format(&client, rate, channels)?;
        let (_, minimum) = client.get_device_period()?;
        let period =
            client.calculate_aligned_period_near(minimum.max(100_000), Some(128), &format)?;
        client.initialize_client(
            &format,
            &Direction::Render,
            &StreamMode::EventsExclusive { period_hns: period },
        )?;
        let event = client.set_get_eventhandle()?;
        let renderer = client.get_audiorenderclient()?;
        let frames = client.get_buffer_size()? as usize;
        Ok(Self {
            client,
            renderer,
            event,
            frames,
            bytes: format.get_blockalign() as usize / channels as usize,
            channels,
            rate,
            started: false,
        })
    }
}
pub(crate) fn native_format(client: &AudioClient, rate: u32, channels: u16) -> Result<WaveFormat> {
    for container in [32, 24] {
        let desired = WaveFormat::new(
            container,
            24,
            &SampleType::Int,
            rate as usize,
            channels as usize,
            None,
        );
        if let Ok(format) = client.is_supported_exclusive_with_quirks(&desired) {
            return Ok(format);
        }
    }
    anyhow::bail!(
        "The device does not accept {rate} Hz, {channels} channels, 24-bit PCM. No resampling was applied"
    )
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = self.client.stop_stream();
    }
}

pub(crate) fn pcm24(sample: f32, bytes: &mut [u8]) {
    // f32 represents 16- and 24-bit integer PCM exactly. Keep 24 valid bits,
    // left-aligned in 32-bit containers when the driver asks for that layout.
    let sample = if sample.is_finite() { sample } else { 0. };
    let value = ((sample as f64 * 8_388_608.)
        .round()
        .clamp(-8_388_608., 8_388_607.) as i32)
        << 8;
    let raw = value.to_le_bytes();
    match bytes.len() {
        4 => bytes.copy_from_slice(&raw),
        3 => bytes.copy_from_slice(&raw[1..]),
        2 => bytes.copy_from_slice(&raw[2..]),
        _ => bytes.fill(0),
    }
}

fn render(
    mut source: SourcesQueueOutput,
    sink: Weak<Sink>,
    device_name: Option<String>,
    state: Arc<Mutex<PlaybackState>>,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    initialize_mta().ok().context("Initialize WASAPI")?;
    let enumerator = DeviceEnumerator::new()?;
    let device = if let Some(name) = device_name {
        enumerator
            .get_device_collection(&Direction::Render)?
            .get_device_with_name(&name)?
    } else {
        enumerator.get_default_device(&Direction::Render)?
    };
    let name = device.get_friendlyname()?;
    let mut endpoint: Option<Endpoint> = None;
    let mut first: Option<f32> = None;
    while !stop.load(Ordering::Relaxed) {
        let Some(sink) = sink.upgrade() else {
            break;
        };
        if sink.empty() || sink.is_paused() {
            endpoint = None;
            if sink.is_paused() && !sink.empty() {
                // Poll Rodio's control wrappers while paused. They yield silence;
                // this lets pending seeks and skips complete without opening a DAC.
                let samples = source.sample_rate() as usize * source.channels() as usize / 100;
                for _ in 0..samples.max(1) {
                    let _ = source.next();
                }
            }
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        if first.is_none() {
            first = source.next();
        }
        let rate = source.sample_rate();
        let channels = source.channels();
        if endpoint
            .as_ref()
            .is_none_or(|e| e.rate != rate || e.channels != channels)
        {
            drop(endpoint.take());
            endpoint = Some(Endpoint::open(&device, rate, channels)?);
            let mut state = state.lock().unwrap();
            state.output = name.clone();
            state.output_rate = rate;
            state.output_channels = channels;
            state.exclusive = true;
        }
        let output = endpoint.as_mut().unwrap();
        let mut data = vec![0u8; output.frames * channels as usize * output.bytes];
        let mut changed = false;
        for bytes in data.chunks_exact_mut(output.bytes) {
            let sample = if let Some(sample) = first.take() {
                Some(sample)
            } else {
                source.next()
            };
            if source.sample_rate() != rate || source.channels() != channels {
                first = sample;
                changed = true;
                break;
            }
            pcm24(sample.unwrap_or(0.), bytes);
        }
        output
            .renderer
            .write_to_device(output.frames, &data, None)?;
        if !output.started {
            output.client.start_stream()?;
            output.started = true;
        }
        output.event.wait_for_event(1000)?;
        if changed {
            endpoint = None;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lossless_integer_pcm_roundtrip() {
        for sample in [i16::MIN, -12345, -1, 0, 1, 12345, i16::MAX] {
            let mut bytes = [0u8; 4];
            pcm24(sample as f32 / 32768., &mut bytes);
            assert_eq!(i32::from_le_bytes(bytes), (sample as i32) << 16);
        }
        for sample in [-8_388_608, -1, 0, 1, 8_388_607] {
            let mut bytes = [0u8; 4];
            pcm24(sample as f32 / 8_388_608., &mut bytes);
            assert_eq!(i32::from_le_bytes(bytes), sample << 8);
        }
    }
}
