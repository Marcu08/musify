use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use crossbeam_channel::{Sender, bounded};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::clock::AudioClock;
use crate::error::AudioError;

#[derive(Debug, Clone, Copy)]
pub struct OutputInfo {
    pub sample_rate: u32,
    pub channels: u16,
}

pub(crate) struct OutputGuard {
    shutdown: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for OutputGuard {
    fn drop(&mut self) {
        let _ = self.shutdown.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(crate) type OutputStart = (OutputGuard, Producer<f32>, OutputInfo);

pub(crate) fn start(clock: Arc<AudioClock>) -> Result<OutputStart, AudioError> {
    let (ready_tx, ready_rx) = bounded(1);
    let (shutdown_tx, shutdown_rx) = bounded(1);
    let thread = thread::Builder::new()
        .name("musify-audio-output".into())
        .spawn(move || match open_output(clock) {
            Ok((stream, producer, info)) => {
                if ready_tx.send(Ok((producer, info))).is_ok() {
                    let _ = shutdown_rx.recv();
                }
                drop(stream);
            }
            Err(error) => {
                let _ = ready_tx.send(Err(error));
            }
        })
        .map_err(|error| AudioError::OutputUnavailable(error.to_string()))?;

    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok((producer, info))) => Ok((
            OutputGuard {
                shutdown: shutdown_tx,
                thread: Some(thread),
            },
            producer,
            info,
        )),
        Ok(Err(error)) => {
            let _ = shutdown_tx.send(());
            let _ = thread.join();
            Err(error)
        }
        Err(_) => {
            let _ = shutdown_tx.send(());
            let _ = thread.join();
            Err(AudioError::OutputUnavailable(
                "the output device did not initialize within 5 seconds".into(),
            ))
        }
    }
}

fn open_output(
    clock: Arc<AudioClock>,
) -> Result<(cpal::Stream, Producer<f32>, OutputInfo), AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| AudioError::OutputUnavailable("no default output device".into()))?;
    let supported = device
        .default_output_config()
        .map_err(|error| AudioError::OutputUnavailable(error.to_string()))?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.config();
    let info = OutputInfo {
        sample_rate: config.sample_rate,
        channels: config.channels,
    };
    let capacity = (info.sample_rate as usize)
        .saturating_mul(2)
        .saturating_mul(2)
        .max(4096);
    let (producer, consumer) = RingBuffer::new(capacity);

    let stream = match sample_format {
        SampleFormat::F32 => build::<f32>(&device, config, consumer, clock),
        SampleFormat::I16 => build::<i16>(&device, config, consumer, clock),
        SampleFormat::U16 => build::<u16>(&device, config, consumer, clock),
        SampleFormat::I32 => build::<i32>(&device, config, consumer, clock),
        other => Err(AudioError::Unsupported(format!(
            "the output device uses an unsupported sample format: {other}"
        ))),
    }?;
    stream
        .play()
        .map_err(|error| AudioError::Output(error.to_string()))?;
    Ok((stream, producer, info))
}

fn build<T>(
    device: &cpal::Device,
    config: StreamConfig,
    mut consumer: Consumer<f32>,
    clock: Arc<AudioClock>,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = (config.channels as usize).max(1);
    let mut gain = f32::from(clock.volume()) / 100.0;
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let target = f32::from(clock.volume()) / 100.0;
                let difference = target - gain;
                if difference.abs() > 0.001 {
                    gain += difference * 0.08;
                } else {
                    gain = target;
                }

                let epoch = clock.epoch.load(Ordering::Acquire);
                if epoch != clock.applied_epoch.load(Ordering::Acquire) {
                    while consumer.pop().is_ok() {}
                    clock.total_played_frames.store(0, Ordering::Release);
                    clock.applied_epoch.store(epoch, Ordering::Release);
                }

                if !clock.playing.load(Ordering::Relaxed) {
                    data.fill(T::EQUILIBRIUM);
                    return;
                }

                let mut frames = 0u64;
                let mut written = 0usize;
                for frame in data.chunks_mut(channels) {
                    if consumer.slots() < 2 {
                        break;
                    }
                    let Ok(left) = consumer.pop() else { break };
                    let Ok(right) = consumer.pop() else { break };
                    let left = (left * gain).clamp(-1.0, 1.0);
                    let right = (right * gain).clamp(-1.0, 1.0);
                    if channels == 1 {
                        frame[0] = T::from_sample((left + right) * 0.5);
                    } else {
                        frame[0] = T::from_sample(left);
                        frame[1] = T::from_sample(right);
                        for sample in frame.iter_mut().skip(2) {
                            *sample = T::EQUILIBRIUM;
                        }
                    }
                    frames += 1;
                    written += channels;
                }

                for sample in data.iter_mut().skip(written) {
                    *sample = T::EQUILIBRIUM;
                }
                if written < data.len() {
                    clock.underruns.fetch_add(1, Ordering::Relaxed);
                }
                clock
                    .total_played_frames
                    .fetch_add(frames, Ordering::Release);
            },
            move |error| tracing::warn!(%error, "audio output stream error"),
            None,
        )
        .map_err(|error| AudioError::Output(error.to_string()))?;
    Ok(stream)
}
