mod alsa;
#[cfg(feature = "host")]
mod host;
mod ring;
mod sfx;
mod sink;
mod stub;
pub mod volume;

pub use alsa::AlsaSink;
#[cfg(feature = "host")]
pub use host::HostAudio;
pub use ring::{ring_capacity, Ring};
pub use sfx::Sfx;
pub use sink::{AudioError, AudioSink};
pub use stub::StubSink;

/// The GBA's own rate. slot plays nothing else, so the device is opened for it before there
/// is a core to ask, and a device that takes it needs no resampling at all.
pub const GBA_HZ: u32 = 32_768;

/// How hard the audio path trades latency for slack.
///
/// What the player feels in a rhythm game is the distance between the beat they *hear* and
/// the press the game *judges*. That is the depth of the ring plus whatever the device
/// buffers, so this is the one knob that moves it.
///
/// Chosen on the shelf and applied when the next cart is inserted: the device buffer is fixed
/// when the PCM is opened, so changing it means reopening the hardware, and on the shelf there
/// is no game to interrupt.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Profile {
    /// The shipped margins, sized against a slow card and a busy scheduler.
    #[default]
    Stable,
    /// The middle: noticeably tighter, still room for a late frame at either end.
    Balanced,
    /// For rhythm games. One video frame of queue plus the device's own floor, which is about
    /// as far as this pipeline can go before a late frame starts costing audio.
    Strict,
}

impl Profile {
    pub const ALL: [Profile; 3] = [Profile::Stable, Profile::Balanced, Profile::Strict];

    /// What `System/audio.txt` holds. Missing or unparsable means `Stable`, the shipped
    /// behaviour: a card edited on a PC must never be able to make the machine click by typo.
    pub fn parse(s: &str) -> Option<Profile> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stable" => Some(Profile::Stable),
            "balanced" => Some(Profile::Balanced),
            "strict" => Some(Profile::Strict),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Stable => "stable",
            Profile::Balanced => "balanced",
            Profile::Strict => "strict",
        }
    }

    // The on-screen label lives with the toast it is drawn from: see `Toast::AudioStable` in
    // `slot-ui`. Keeping it here too would be a second copy of the same three strings.

    pub fn next(self) -> Profile {
        match self {
            Profile::Stable => Profile::Balanced,
            Profile::Balanced => Profile::Strict,
            Profile::Strict => Profile::Stable,
        }
    }

    pub fn prev(self) -> Profile {
        match self {
            Profile::Stable => Profile::Strict,
            Profile::Balanced => Profile::Stable,
            Profile::Strict => Profile::Balanced,
        }
    }

    /// Where the DRC parks the ring, in video frames of audio. `Stable` reproduces the shipped
    /// steady state exactly (`capacity / 2`); the others are fixed counts, which is what makes
    /// them independent of how big the ring happens to be.
    pub fn target(self, capacity_frames: usize) -> usize {
        match self {
            Profile::Stable => capacity_frames / 2,
            Profile::Balanced => capacity_frames * 3 / 8,
            Profile::Strict => capacity_frames / 4,
        }
    }

    /// What ALSA is asked to buffer, in microseconds.
    pub fn latency_us(self) -> u32 {
        match self {
            Profile::Stable => 40_000,
            Profile::Balanced => 24_000,
            Profile::Strict => 16_000,
        }
    }

    /// Frames per device write. The device needs at least two periods, so this is what sets
    /// the floor under `latency_us`: at the GBA's rate 512 frames is 15.6 ms and 256 is 7.8 ms.
    pub fn period_frames(self) -> usize {
        match self {
            Profile::Stable => 512,
            Profile::Balanced => 256,
            Profile::Strict => 256,
        }
    }
}

/// The sink this build talks to: cpal on a desktop, ALSA on the device. A sink that fails to
/// open is not a failure to boot either way, since a ring nothing drains still lets the
/// emulator run.
#[cfg(feature = "host")]
pub fn open_sink() -> Box<dyn AudioSink> {
    Box::new(HostAudio::new())
}

#[cfg(not(feature = "host"))]
pub fn open_sink() -> Box<dyn AudioSink> {
    Box::new(AlsaSink::new())
}
