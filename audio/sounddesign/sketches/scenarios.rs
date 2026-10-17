//! The scenarios: scripted ones, which move the controls like the sliders, and the content
//! modules, rendered by the real renderer.

use std::sync::Arc;

use nova_audio::strata;
use nova_audio::{Preset, fathom};
use nova_core::{Bus, Controls, Features, HUES};
use nova_voxel::{MODULE_X_RES, MODULE_Y_RES, MODULE_Z_RES, Renderer, Settings, all_content_names};

/// The display's frame time.
pub const FRAME_SECONDS: f32 = 0.04;

/// Seconds of each content module's scenario.
const MODULE_SECONDS: f32 = 45.0;

/// The image of scripted scenarios, neutral: gray light, half the voxels lit, centred and
/// spread evenly, with activity in the middle of the range the sound follows (0.003 to 0.3
/// on a logarithmic scale).
const IMAGE: Features = Features {
    level: 0.25,
    balance: 0.0,
    width: 1.0,
    height: 0.5,
    density: 0.5,
    activity: 0.03,
    hues: [0.0; HUES],
};

/// A display frame: what the engine plays from until the next one, as the bus carries it.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// As set, with the volume up.
    pub controls: Controls,
    /// The tide clock.
    pub clock: f64,
    pub light: Features,
}

pub struct Scenario {
    pub name: String,
    pub description: String,
    pub preset: Preset,
    pub frames: Vec<Frame>,
}

/// The sliders a scripted scenario moves, and the light it hands over. Every control
/// starts at 0.5 and the palette at the first one; the sound moves them by the global
/// tides, as it does live. The light starts neutral (`IMAGE`).
pub struct Script<'a> {
    names: &'a [String],
    /// Position in the palettes.
    pub palette: usize,
    heat: f32,
    flow: f32,
    form: f32,
    void: f32,
    image: Features,
}

impl Script<'_> {
    /// How many palettes there are.
    pub fn palettes(&self) -> usize {
        self.names.len()
    }

    pub fn set_palette(&mut self, name: &str) {
        self.palette = self
            .names
            .iter()
            .position(|known| known == name)
            .unwrap_or_else(|| panic!("no palette named {name}"));
    }

    pub fn set_heat(&mut self, value: f32) {
        self.heat = value;
    }

    pub fn set_flow(&mut self, value: f32) {
        self.flow = value;
    }

    pub fn set_form(&mut self, value: f32) {
        self.form = value;
    }

    pub fn set_void(&mut self, value: f32) {
        self.void = value;
    }

    /// Where the light sits, from 0 at the bottom to 1 at the top.
    pub fn set_height(&mut self, value: f32) {
        self.image.height = value;
    }

    /// The share of lit voxels.
    pub fn set_density(&mut self, value: f32) {
        self.image.density = value;
    }

    /// Where the light sits, from −1 on the left to 1 on the right.
    pub fn set_balance(&mut self, value: f32) {
        self.image.balance = value;
    }
}

/// A scripted scenario before its frames are made.
struct Spec {
    name: &'static str,
    description: &'static str,
    seconds: f32,
    preset: Preset,
    /// Sets the controls at a point of the scenario, from 0 to 1.
    set: fn(f32, &mut Script),
}

fn scenario(
    name: &'static str,
    description: &'static str,
    seconds: f32,
    preset: Preset,
    set: fn(f32, &mut Script),
) -> Spec {
    Spec {
        name,
        description,
        seconds,
        preset,
        set,
    }
}

/// Frames for `seconds`, with the controls `set` makes at each point from 0 to 1. The tide
/// clock stops at Flow 0, as on the display.
pub fn script(names: &[String], seconds: f32, set: impl Fn(f32, &mut Script)) -> Vec<Frame> {
    let count = (seconds / FRAME_SECONDS).round() as usize;
    let mut script = Script {
        names,
        palette: 0,
        heat: 0.5,
        flow: 0.5,
        form: 0.5,
        void: 0.5,
        image: IMAGE,
    };
    let mut clock = 0.0;
    (0..count)
        .map(|frame| {
            set(frame as f32 / (count - 1).max(1) as f32, &mut script);
            if script.flow > 0.0 {
                clock += FRAME_SECONDS as f64;
            }
            Frame {
                controls: Controls {
                    palette: script.palette,
                    heat: script.heat,
                    flow: script.flow,
                    form: script.form,
                    void: script.void,
                    ..loud()
                },
                clock,
                light: script.image,
            }
        })
        .collect()
}

/// The content's name in lower case, with underscores: `Orion's Belt` → `orions_belt`.
fn slug(content: &str) -> String {
    content
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            'a'..='z' => Some(c),
            ' ' => Some('_'),
            'ä' => Some('a'),
            _ => None,
        })
        .collect()
}

/// Every control in the middle and the first palette, with the volume up.
fn loud() -> Controls {
    Controls {
        volume: 1.0,
        ..Controls::default()
    }
}

/// Frames of the content module at `index` of `all_content_names()`, on a single module with
/// every control at 0.5 and the first palette, for `seconds`, as the renderer leaves them on
/// the bus.
pub fn module(index: usize, seconds: f32) -> Vec<Frame> {
    let bus = Arc::new(Bus::new(&loud()));
    let mut renderer = Renderer::new((MODULE_X_RES, MODULE_Y_RES, MODULE_Z_RES), Arc::clone(&bus));
    let settings = Settings {
        content: index,
        ..Settings::default()
    };
    (0..(seconds / FRAME_SECONDS).round() as usize)
        .map(|_| {
            renderer.render_step(&settings, FRAME_SECONDS);
            Frame {
                controls: bus.controls(),
                clock: bus.clock(),
                light: bus.light(),
            }
        })
        .collect()
}

/// Every scenario: Strata's, then Fathom's, each with its scripted ones first, then one per
/// content module, named by the module.
pub fn all(names: &[String]) -> Vec<Scenario> {
    let modules: Vec<(String, Vec<Frame>)> = all_content_names()
        .iter()
        .enumerate()
        .map(|(index, content)| (content.clone(), module(index, MODULE_SECONDS)))
        .collect();
    let mut scenarios = Vec::new();
    for (specs, prefix, preset) in [
        (
            strata_scenarios(names.len()),
            "strata_",
            Preset::Strata(strata::Preset::DEFAULT),
        ),
        (
            fathom_scenarios(names.len()),
            "fathom_",
            Preset::Fathom(fathom::Preset::DEFAULT),
        ),
    ] {
        let first = specs.len() + 1;
        for spec in specs {
            scenarios.push(Scenario {
                name: spec.name.into(),
                description: spec.description.into(),
                preset: spec.preset,
                frames: script(names, spec.seconds, spec.set),
            });
        }
        for (index, (content, frames)) in modules.iter().enumerate() {
            scenarios.push(Scenario {
                name: format!("{prefix}{:02}_module_{}", first + index, slug(content)),
                description: format!("{content} with every control at 0.5"),
                preset,
                frames: frames.clone(),
            });
        }
    }
    scenarios
}

/// Strata's scripted scenarios, named `strata_…`.
fn strata_scenarios(palettes: usize) -> Vec<Spec> {
    let euclidean = Preset::Strata(strata::Preset::DEFAULT);
    let polymeter = Preset::Strata(strata::Preset::POLYMETER);
    vec![
        scenario(
            "strata_01_cold_to_warm",
            "Pure drone (Form 0.05), Heat from 0 to 1",
            90.0,
            euclidean,
            |t, state| {
                state.set_form(0.05);
                state.set_heat(t);
            },
        ),
        scenario(
            "strata_02_ambient_to_rhythmic_euclidean",
            "Form from 0 to 1, Euclidean patterns on a 16-step bar",
            120.0,
            euclidean,
            |t, state| state.set_form(t),
        ),
        scenario(
            "strata_03_ambient_to_rhythmic_polymeter",
            "Form from 0 to 1, loops of 16, 12 and 7 steps",
            120.0,
            polymeter,
            |t, state| state.set_form(t),
        ),
        scenario(
            "strata_04_void_sweep",
            "Void from 0 to 1 (Form 0.3)",
            60.0,
            euclidean,
            |t, state| {
                state.set_form(0.3);
                state.set_void(t);
            },
        ),
        scenario(
            "strata_05_flow_sweep",
            "Flow from 0 to 1 (Form 0.6)",
            60.0,
            euclidean,
            |t, state| {
                state.set_form(0.6);
                state.set_flow(t);
            },
        ),
        scenario(
            "strata_06_palettes",
            "Every palette for 10 s (Form 0.35)",
            10.0 * palettes as f32,
            euclidean,
            |t, state| {
                let count = state.palettes();
                state.palette = ((t * count as f32) as usize).min(count - 1);
                state.set_form(0.35);
            },
        ),
        scenario(
            "strata_07_cold_ambient",
            "Antarctica Freeze, Heat 0.1, Form 0.1, Flow 0.4, Void 0.6",
            60.0,
            euclidean,
            |_, state| {
                state.set_palette("Antarctica Freeze");
                state.set_heat(0.1);
                state.set_form(0.1);
                state.set_flow(0.4);
                state.set_void(0.6);
            },
        ),
        scenario(
            "strata_08_warm_ambient",
            "Sahara Drift, Heat 0.9, Form 0.1, Flow 0.4, Void 0.4",
            60.0,
            euclidean,
            |_, state| {
                state.set_palette("Sahara Drift");
                state.set_heat(0.9);
                state.set_form(0.1);
                state.set_flow(0.4);
                state.set_void(0.4);
            },
        ),
        scenario(
            "strata_09_cold_rhythmic",
            "Kiruna Glow, Heat 0.15, Form 0.8, Flow 0.6 (Euclidean)",
            60.0,
            euclidean,
            |_, state| {
                state.set_palette("Kiruna Glow");
                state.set_heat(0.15);
                state.set_form(0.8);
                state.set_flow(0.6);
            },
        ),
        scenario(
            "strata_10_warm_rhythmic",
            "Shenzhen Lantern Flicker, Heat 0.85, Form 0.8, Flow 0.55 (polymeter)",
            60.0,
            polymeter,
            |_, state| {
                state.set_palette("Shenzhen Lantern Flicker");
                state.set_heat(0.85);
                state.set_form(0.8);
                state.set_flow(0.55);
            },
        ),
        scenario(
            "strata_11_harmonic_motion",
            "Belize Ripple, Heat 0.6, Form 0.3, Flow 0.6 for 10 minutes: the bass moves about \
             every minute, a chord tone changes its octave halfway",
            600.0,
            euclidean,
            |_, state| {
                state.set_palette("Belize Ripple");
                state.set_heat(0.6);
                state.set_form(0.3);
                state.set_flow(0.6);
            },
        ),
        scenario(
            "strata_12_height_sweep",
            "The light from the bottom to the top (Form 0.3)",
            60.0,
            euclidean,
            |t, state| {
                state.set_form(0.3);
                state.set_height(t);
            },
        ),
    ]
}

/// Fathom's scripted scenarios, named `fathom_…`.
fn fathom_scenarios(palettes: usize) -> Vec<Spec> {
    let fathom = Preset::Fathom(fathom::Preset::DEFAULT);
    let long = Preset::Fathom(fathom::Preset::DEFAULT.with_throws(fathom::Throws::LONG));
    vec![
        scenario(
            "fathom_01_cold_to_warm",
            "Stabs (Form 0.75), Heat from 0 to 1",
            90.0,
            fathom,
            |t, state| {
                state.set_form(0.75);
                state.set_heat(t);
            },
        ),
        scenario(
            "fathom_02_ambient_to_rhythmic",
            "Form from 0 to 1: drone, swells, stabs, then the kick",
            120.0,
            fathom,
            |t, state| state.set_form(t),
        ),
        scenario(
            "fathom_03_void_sweep",
            "Void from 0 to 1 (Form 0.75)",
            60.0,
            fathom,
            |t, state| {
                state.set_form(0.75);
                state.set_void(t);
            },
        ),
        scenario(
            "fathom_04_flow_sweep",
            "Flow from 0 to 1 (Form 0.75)",
            60.0,
            fathom,
            |t, state| {
                state.set_form(0.75);
                state.set_flow(t);
            },
        ),
        scenario(
            "fathom_05_palettes",
            "Every palette for 10 s (Form 0.6)",
            10.0 * palettes as f32,
            fathom,
            |t, state| {
                let count = state.palettes();
                state.palette = ((t * count as f32) as usize).min(count - 1);
                state.set_form(0.6);
            },
        ),
        scenario(
            "fathom_06_cold_ambient",
            "Antarctica Freeze, Heat 0.1, Form 0.1, Flow 0.4, Void 0.6",
            60.0,
            fathom,
            |_, state| {
                state.set_palette("Antarctica Freeze");
                state.set_heat(0.1);
                state.set_form(0.1);
                state.set_flow(0.4);
                state.set_void(0.6);
            },
        ),
        scenario(
            "fathom_07_warm_ambient",
            "Sahara Drift, Heat 0.9, Form 0.1, Flow 0.4, Void 0.4",
            60.0,
            fathom,
            |_, state| {
                state.set_palette("Sahara Drift");
                state.set_heat(0.9);
                state.set_form(0.1);
                state.set_flow(0.4);
                state.set_void(0.4);
            },
        ),
        scenario(
            "fathom_08_cold_rhythmic",
            "Kiruna Glow, Heat 0.15, Form 0.8, Flow 0.6",
            60.0,
            fathom,
            |_, state| {
                state.set_palette("Kiruna Glow");
                state.set_heat(0.15);
                state.set_form(0.8);
                state.set_flow(0.6);
            },
        ),
        scenario(
            "fathom_09_warm_rhythmic",
            "Shenzhen Lantern Flicker, Heat 0.85, Form 0.8, Flow 0.55",
            60.0,
            fathom,
            |_, state| {
                state.set_palette("Shenzhen Lantern Flicker");
                state.set_heat(0.85);
                state.set_form(0.8);
                state.set_flow(0.55);
            },
        ),
        scenario(
            "fathom_10_harmonic_motion",
            "Belize Ripple, Heat 0.6, Form 0.6, Flow 0.6 for 10 minutes: the bass moves about \
             every minute and throws the stabs, the filter opens while it is away",
            600.0,
            fathom,
            |_, state| {
                state.set_palette("Belize Ripple");
                state.set_heat(0.6);
                state.set_form(0.6);
                state.set_flow(0.6);
            },
        ),
        scenario(
            "fathom_11_long_listening",
            "Throws for long listening: Belize Ripple, Form 0.75, Flow 0.5 for 3 minutes",
            180.0,
            long,
            |_, state| {
                state.set_palette("Belize Ripple");
                state.set_form(0.75);
            },
        ),
        scenario(
            "fathom_12_height_sweep",
            "The light from the bottom to the top (Form 0.75)",
            60.0,
            fathom,
            |t, state| {
                state.set_form(0.75);
                state.set_height(t);
            },
        ),
        scenario(
            "fathom_13_density_sweep",
            "From all voxels lit to none (Form 0.75)",
            60.0,
            fathom,
            |t, state| {
                state.set_form(0.75);
                state.set_density(1.0 - t);
            },
        ),
        scenario(
            "fathom_14_balance_sweep",
            "The light from the left to the right (Form 0.75)",
            60.0,
            fathom,
            |t, state| {
                state.set_form(0.75);
                state.set_balance(2.0 * t - 1.0);
            },
        ),
    ]
}
