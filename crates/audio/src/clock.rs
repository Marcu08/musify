use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};

/// Values shared with the real-time callback. No locks, no allocations.
#[derive(Debug)]
pub(crate) struct AudioClock {
    pub playing: AtomicBool,
    pub volume: AtomicU16,
    pub total_played_frames: AtomicU64,
    pub epoch: AtomicU64,
    pub applied_epoch: AtomicU64,
    pub underruns: AtomicU64,
}

impl AudioClock {
    pub fn new(volume: u16) -> Self {
        Self {
            playing: AtomicBool::new(false),
            volume: AtomicU16::new(volume.min(100)),
            total_played_frames: AtomicU64::new(0),
            epoch: AtomicU64::new(0),
            applied_epoch: AtomicU64::new(0),
            underruns: AtomicU64::new(0),
        }
    }

    pub fn volume(&self) -> u16 {
        self.volume.load(Ordering::Relaxed)
    }

    pub fn total_played_frames(&self) -> u64 {
        self.total_played_frames.load(Ordering::Acquire)
    }

    pub fn underruns(&self) -> u64 {
        self.underruns.load(Ordering::Relaxed)
    }
}
