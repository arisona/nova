//! The bus: what the parts share while Nova plays. The host puts the controls on it as the
//! user sets them, and the instrument the selected content plays; the display adds the tide clock and the features of its light with every
//! frame; the sound reads all of it. Each part also leaves its status there for the host.
//!
//! Lock-free for the values, so the audio callback can read them; values may come from
//! different moments, which the readers' smoothing hides. Statuses change rarely and are
//! read and written only outside the audio callback.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};

use crate::{Controls, Features, HUES};

/// How a part of Nova is doing, with a message for people.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum Status {
    #[default]
    Unknown,
    Ok(String),
    Err(String),
}

/// A part of Nova that reports its status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The voxel display, on the hardware or in the simulator.
    Display,
    /// The sound.
    Sound,
}

#[derive(Debug, Default)]
struct Float(AtomicU32);

impl Float {
    fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Ordering::Relaxed);
    }

    fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Default)]
pub struct Bus {
    brightness: Float,
    volume: Float,
    palette: AtomicUsize,
    heat: Float,
    flow: Float,
    form: Float,
    void: Float,
    instrument: AtomicUsize,
    clock: AtomicU64,
    level: Float,
    balance: Float,
    width: Float,
    height: Float,
    density: Float,
    activity: Float,
    hues: [Float; HUES],
    display: Mutex<Status>,
    sound: Mutex<Status>,
}

impl Bus {
    /// A bus with the given controls, the tide clock at 0 and the light of a dark image.
    pub fn new(controls: &Controls) -> Self {
        let bus = Self::default();
        bus.set_controls(controls);
        bus.set_light(&Features::default());
        bus
    }

    /// The controls as the user set them.
    pub fn set_controls(&self, controls: &Controls) {
        self.brightness.set(controls.brightness);
        self.volume.set(controls.volume);
        self.palette.store(controls.palette, Ordering::Relaxed);
        self.heat.set(controls.heat);
        self.flow.set(controls.flow);
        self.form.set(controls.form);
        self.void.set(controls.void);
    }

    pub fn controls(&self) -> Controls {
        Controls {
            brightness: self.brightness.get(),
            volume: self.volume.get(),
            palette: self.palette.load(Ordering::Relaxed),
            heat: self.heat.get(),
            flow: self.flow.get(),
            form: self.form.get(),
            void: self.void.get(),
        }
    }

    /// The instrument to play, a position in the sound's list of instruments
    /// (`nova_audio::INSTRUMENTS`). The sound mixes over to a new one.
    pub fn set_instrument(&self, instrument: usize) {
        self.instrument.store(instrument, Ordering::Relaxed);
    }

    pub fn instrument(&self) -> usize {
        self.instrument.load(Ordering::Relaxed)
    }

    /// The tide clock, in seconds (see `tides::TideClock`).
    pub fn set_clock(&self, seconds: f64) {
        self.clock.store(seconds.to_bits(), Ordering::Relaxed);
    }

    pub fn clock(&self) -> f64 {
        f64::from_bits(self.clock.load(Ordering::Relaxed))
    }

    /// The features of the display's light.
    pub fn set_light(&self, features: &Features) {
        self.level.set(features.level);
        self.balance.set(features.balance);
        self.width.set(features.width);
        self.height.set(features.height);
        self.density.set(features.density);
        self.activity.set(features.activity);
        for (slot, &share) in self.hues.iter().zip(&features.hues) {
            slot.set(share);
        }
    }

    pub fn light(&self) -> Features {
        Features {
            level: self.level.get(),
            balance: self.balance.get(),
            width: self.width.get(),
            height: self.height.get(),
            density: self.density.get(),
            activity: self.activity.get(),
            hues: self.hues.each_ref().map(Float::get),
        }
    }

    /// Leaves a part's status for the host. Not for the audio callback.
    pub fn report(&self, part: Part, status: Status) {
        *self.slot(part).lock().unwrap() = status;
    }

    pub fn status(&self, part: Part) -> Status {
        self.slot(part).lock().unwrap().clone()
    }

    fn slot(&self, part: Part) -> &Mutex<Status> {
        match part {
            Part::Display => &self.display,
            Part::Sound => &self.sound,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_carries_controls_instrument_clock_light_and_statuses() {
        let controls = Controls {
            brightness: 0.1,
            volume: 0.4,
            palette: 2,
            heat: 0.3,
            flow: 0.5,
            form: 0.6,
            void: 0.7,
        };
        let bus = Bus::new(&controls);
        assert_eq!(bus.controls(), controls);
        assert_eq!(bus.clock(), 0.0);
        assert_eq!(bus.instrument(), 0);
        bus.set_instrument(2);
        assert_eq!(bus.instrument(), 2);
        assert_eq!(bus.light(), Features::default());
        bus.set_clock(1_234.567_891_234);
        assert_eq!(bus.clock(), 1_234.567_891_234);
        let mut light = Features {
            level: 0.2,
            balance: -0.3,
            activity: 0.05,
            ..Features::default()
        };
        light.hues[3] = 1.0;
        bus.set_light(&light);
        assert_eq!(bus.light(), light);
        assert_eq!(bus.status(Part::Sound), Status::Unknown);
        bus.report(Part::Sound, Status::Ok("Audio ready".into()));
        assert_eq!(bus.status(Part::Sound), Status::Ok("Audio ready".into()));
        assert_eq!(bus.status(Part::Display), Status::Unknown);
    }
}
