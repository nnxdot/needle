//! Sound effects from plugins, played after the equalizer on shared output.
//!
//! A plugin's `effects()` function describes each effect. An effect is either:
//! - a chain of Needle's built-in blocks (filters, compressor, limiter, delay, reverb, chorus,
//!   tremolo, saturation, stereo width, pan, gain), whose settings can follow the effect's
//!   sliders, or
//! - a WebAssembly module with its own DSP code, run in a sandbox with no imports, a memory cap,
//!   and a fuel budget per block, so it cannot reach the computer or hang playback.
//!
//! The listener's chain of effects (which ones, in what order, on or off, slider values) lives
//! in [`crate::dsp::Dsp::effects`]; the definitions live in the [`Registry`] that the plugin host
//! fills with the effects of turned-on plugins.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap},
    f64::consts::PI,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

/// Frames processed at once. WebAssembly effects see blocks of at most this many frames.
pub const BLOCK_FRAMES: usize = 512;
const MAX_PARAMS: usize = 16;
const MAX_BLOCKS: usize = 16;
const MAX_EFFECTS: usize = 16;
const MAX_WASM_BYTES: usize = 8 << 20;
const WASM_MEMORY: usize = 32 << 20;

/// One effect in the listener's chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSlot {
    /// Tells two copies of the same effect apart, and keeps an effect's state across edits.
    pub uid: String,
    pub plugin: String,
    pub effect: String,
    pub on: bool,
    /// Slider values by parameter id; missing ones use the effect's starting value.
    #[serde(default)]
    pub params: BTreeMap<String, f32>,
}
impl EffectSlot {
    /// A new, turned-on copy of an effect with its starting slider values.
    pub fn new(plugin: &str, effect: &str) -> Self {
        Self {
            uid: uuid::Uuid::new_v4().to_string(),
            plugin: plugin.into(),
            effect: effect.into(),
            on: true,
            params: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: String,
    pub name: String,
    pub min: f32,
    pub max: f32,
    /// The starting value.
    pub value: f32,
    #[serde(default)]
    pub step: Option<f32>,
    /// Shown after the value, such as "dB", "ms", "Hz", or "%" (shown as 0–100).
    #[serde(default)]
    pub unit: String,
}
impl ParamDef {
    pub fn step(&self) -> f32 {
        self.step
            .filter(|s| *s > 0.)
            .unwrap_or(((self.max - self.min) / 100.).max(f32::EPSILON))
    }
    /// The value as people read it.
    pub fn display(&self, value: f32) -> String {
        match self.unit.as_str() {
            "%" => format!("{:.0}%", value * 100.),
            "" => format!("{value:.2}"),
            unit if value.abs() >= 100. => format!("{value:.0} {unit}"),
            unit => format!("{value:.1} {unit}"),
        }
    }
}

/// A block setting: a number, or the value of one of the effect's sliders.
#[derive(Clone, Debug, PartialEq)]
pub enum Setting {
    Fixed(f32),
    Param(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockSpec {
    pub kind: BlockKind,
    /// The filter shape, for filter blocks.
    pub shape: Option<Shape>,
    pub settings: BTreeMap<String, Setting>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Gain,
    Filter,
    Compressor,
    Limiter,
    Delay,
    Reverb,
    Chorus,
    Tremolo,
    Saturate,
    Width,
    Pan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Peak,
    Lowshelf,
    Highshelf,
}

/// Each block's settings: name, starting value, lowest, highest.
fn block_settings(kind: BlockKind) -> &'static [(&'static str, f32, f32, f32)] {
    match kind {
        BlockKind::Gain => &[("db", 0., -60., 24.)],
        BlockKind::Filter => &[
            ("frequency", 1000., 10., 24000.),
            ("q", 0.707, 0.1, 20.),
            ("gain", 0., -24., 24.),
        ],
        BlockKind::Compressor => &[
            ("threshold", -18., -60., 0.),
            ("ratio", 4., 1., 20.),
            ("attack", 10., 0.1, 200.),
            ("release", 120., 5., 2000.),
            ("makeup", 0., -12., 24.),
        ],
        BlockKind::Limiter => &[("ceiling", -1., -24., 0.), ("release", 80., 5., 2000.)],
        BlockKind::Delay => &[
            ("time", 350., 1., 2000.),
            ("feedback", 0.35, 0., 0.95),
            ("mix", 0.3, 0., 1.),
        ],
        BlockKind::Reverb => &[
            ("size", 0.6, 0., 1.),
            ("damping", 0.4, 0., 1.),
            ("mix", 0.25, 0., 1.),
            ("width", 1., 0., 1.),
        ],
        BlockKind::Chorus => &[
            ("rate", 0.8, 0.05, 5.),
            ("depth", 3., 0., 10.),
            ("mix", 0.4, 0., 1.),
        ],
        BlockKind::Tremolo => &[("rate", 5., 0.1, 20.), ("depth", 0.5, 0., 1.)],
        BlockKind::Saturate => &[("drive", 6., 0., 36.), ("mix", 1., 0., 1.)],
        BlockKind::Width => &[("amount", 1., 0., 2.)],
        BlockKind::Pan => &[("position", 0., -1., 1.)],
    }
}

impl BlockKind {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "gain" => Self::Gain,
            "filter" => Self::Filter,
            "compressor" => Self::Compressor,
            "limiter" => Self::Limiter,
            "delay" => Self::Delay,
            "reverb" => Self::Reverb,
            "chorus" => Self::Chorus,
            "tremolo" => Self::Tremolo,
            "saturate" => Self::Saturate,
            "width" => Self::Width,
            "pan" => Self::Pan,
            _ => return None,
        })
    }
}
impl Shape {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "lowpass" => Self::Lowpass,
            "highpass" => Self::Highpass,
            "bandpass" => Self::Bandpass,
            "notch" => Self::Notch,
            "peak" => Self::Peak,
            "lowshelf" => Self::Lowshelf,
            "highshelf" => Self::Highshelf,
            _ => return None,
        })
    }
}

/// Compiled WebAssembly DSP code, shared by every song that plays it.
pub struct WasmCode {
    engine: wasmi::Engine,
    module: wasmi::Module,
}
impl std::fmt::Debug for WasmCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WasmCode")
    }
}
impl WasmCode {
    pub fn compile(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_WASM_BYTES {
            bail!("The WebAssembly file is larger than 8 MB");
        }
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        let engine = wasmi::Engine::new(&config);
        let module = wasmi::Module::new(&engine, bytes)
            .map_err(|e| anyhow::anyhow!("Not valid WebAssembly: {e}"))?;
        if let Some(import) = module.imports().next() {
            bail!(
                "DSP modules cannot import anything, but this one imports {}.{}",
                import.module(),
                import.name()
            );
        }
        let code = Self { engine, module };
        // Fail now, not while music plays, when the module lacks what Needle calls.
        WasmRunner::new(&code, 48000, 2)?;
        Ok(code)
    }
}

#[derive(Debug)]
pub enum EffectKind {
    Blocks(Vec<BlockSpec>),
    Wasm(WasmCode),
}

#[derive(Debug)]
pub struct EffectDef {
    pub plugin: String,
    pub plugin_name: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub params: Vec<ParamDef>,
    pub kind: EffectKind,
}
impl EffectDef {
    /// The slider values for `slot`, filled in and kept in range.
    pub fn values(&self, slot: &EffectSlot) -> Vec<f32> {
        self.params
            .iter()
            .map(|p| {
                slot.params
                    .get(&p.id)
                    .copied()
                    .filter(|v| v.is_finite())
                    .unwrap_or(p.value)
                    .clamp(p.min, p.max)
            })
            .collect()
    }
    pub fn is_wasm(&self) -> bool {
        matches!(self.kind, EffectKind::Wasm(_))
    }
}

/// The effects of turned-on plugins, shared by the plugin host, the player, and the interface.
#[derive(Default)]
pub struct Registry {
    effects: Mutex<Vec<Arc<EffectDef>>>,
    version: AtomicU64,
    /// Effects that stopped while playing, and why, by (plugin, effect).
    failures: Mutex<HashMap<(String, String), String>>,
}
impl Registry {
    pub fn set(&self, effects: Vec<Arc<EffectDef>>) {
        *self.effects.lock().unwrap_or_else(|p| p.into_inner()) = effects;
        self.failures
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        self.version.fetch_add(1, Ordering::Release);
    }
    pub fn all(&self) -> Vec<Arc<EffectDef>> {
        self.effects
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
    pub fn find(&self, plugin: &str, effect: &str) -> Option<Arc<EffectDef>> {
        self.effects
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .find(|e| e.plugin == plugin && e.id == effect)
            .cloned()
    }
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }
    pub fn failure(&self, plugin: &str, effect: &str) -> Option<String> {
        self.failures
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&(plugin.to_string(), effect.to_string()))
            .cloned()
    }
    fn fail(&self, def: &EffectDef, message: String) {
        // Called from the audio thread: never wait for the lock.
        if let Ok(mut failures) = self.failures.try_lock() {
            failures.insert((def.plugin.clone(), def.id.clone()), message);
        }
    }
}

// ---------------------------------------------------------------- reading definitions

#[derive(Deserialize)]
struct RawEffect {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    params: Vec<RawParam>,
    #[serde(default)]
    blocks: Vec<BTreeMap<String, serde_json::Value>>,
    #[serde(default)]
    wasm: Option<String>,
}
#[derive(Deserialize)]
struct RawParam {
    id: String,
    name: String,
    min: f64,
    max: f64,
    value: f64,
    #[serde(default)]
    step: Option<f64>,
    #[serde(default)]
    unit: String,
}

fn plain_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Read what a plugin's `effects()` returned. `read` loads a file from the plugin's folder.
pub fn parse(
    plugin: &str,
    plugin_name: &str,
    value: serde_json::Value,
    read: impl Fn(&str) -> Result<Vec<u8>>,
) -> Result<Vec<EffectDef>> {
    let raw: Vec<RawEffect> = serde_json::from_value(value).context(
        "effects() must return a list of maps with at least an id and a name; slider maps need id, name, min, max, and value",
    )?;
    if raw.len() > MAX_EFFECTS {
        bail!("A plugin can add at most {MAX_EFFECTS} effects");
    }
    let mut seen = std::collections::HashSet::new();
    raw.into_iter()
        .map(|effect| {
            let what = format!("Effect \"{}\"", effect.id);
            if !plain_id(&effect.id) {
                bail!("{what}: the id must use only letters, digits, - and _");
            }
            if !seen.insert(effect.id.clone()) {
                bail!("{what} is listed twice");
            }
            if effect.params.len() > MAX_PARAMS {
                bail!("{what} has more than {MAX_PARAMS} sliders");
            }
            let mut params = vec![];
            for p in effect.params {
                if !plain_id(&p.id) || params.iter().any(|q: &ParamDef| q.id == p.id) {
                    bail!("{what}: slider \"{}\" needs its own plain id", p.id);
                }
                if !(p.min.is_finite() && p.max.is_finite() && p.value.is_finite())
                    || p.min >= p.max
                {
                    bail!("{what}: slider \"{}\" needs min below max", p.id);
                }
                params.push(ParamDef {
                    id: p.id,
                    name: p.name,
                    min: p.min as f32,
                    max: p.max as f32,
                    value: (p.value as f32).clamp(p.min as f32, p.max as f32),
                    step: p.step.map(|s| s as f32),
                    unit: p.unit,
                });
            }
            let kind = match (effect.wasm, effect.blocks.is_empty()) {
                (Some(_), false) => bail!("{what}: use blocks or wasm, not both"),
                (None, true) => bail!("{what} needs blocks or a wasm file"),
                (Some(file), true) => {
                    let mut bytes =
                        read(&file).with_context(|| format!("{what}: cannot read {file}"))?;
                    // WebAssembly text, for small effects written by hand.
                    if file.to_lowercase().ends_with(".wat") {
                        bytes = wat::parse_bytes(&bytes)
                            .map_err(|e| {
                                anyhow::anyhow!("{what}: {file} is not valid WebAssembly text: {e}")
                            })?
                            .into_owned();
                    }
                    EffectKind::Wasm(WasmCode::compile(&bytes).with_context(|| what.clone())?)
                }
                (None, false) => {
                    if effect.blocks.len() > MAX_BLOCKS {
                        bail!("{what} has more than {MAX_BLOCKS} blocks");
                    }
                    EffectKind::Blocks(
                        effect
                            .blocks
                            .into_iter()
                            .map(|b| parse_block(b, &params).with_context(|| what.clone()))
                            .collect::<Result<_>>()?,
                    )
                }
            };
            Ok(EffectDef {
                plugin: plugin.into(),
                plugin_name: plugin_name.into(),
                id: effect.id,
                name: effect.name,
                description: effect.description,
                params,
                kind,
            })
        })
        .collect()
}

fn parse_block(
    mut raw: BTreeMap<String, serde_json::Value>,
    params: &[ParamDef],
) -> Result<BlockSpec> {
    let kind_name = raw
        .remove("kind")
        .and_then(|v| v.as_str().map(str::to_string))
        .context("Each block needs a kind, such as \"filter\" or \"reverb\"")?;
    let kind = BlockKind::parse(&kind_name).with_context(|| {
        format!("Unknown block kind \"{kind_name}\". Use gain, filter, compressor, limiter, delay, reverb, chorus, tremolo, saturate, width, or pan")
    })?;
    let shape = match raw.remove("shape") {
        Some(value) if kind == BlockKind::Filter => {
            let name = value.as_str().unwrap_or_default().to_string();
            Some(Shape::parse(&name).with_context(|| {
                format!("Unknown filter shape \"{name}\". Use lowpass, highpass, bandpass, notch, peak, lowshelf, or highshelf")
            })?)
        }
        Some(_) => bail!("Only filter blocks have a shape"),
        None if kind == BlockKind::Filter => bail!("Filter blocks need a shape"),
        None => None,
    };
    let known = block_settings(kind);
    let mut settings = BTreeMap::new();
    for (name, value) in raw {
        if !known.iter().any(|(k, ..)| *k == name) {
            let names: Vec<&str> = known.iter().map(|(k, ..)| *k).collect();
            bail!(
                "A {kind_name} block has no setting \"{name}\". It has: {}",
                names.join(", ")
            );
        }
        let setting = match &value {
            serde_json::Value::Number(n) => Setting::Fixed(n.as_f64().unwrap_or(0.) as f32),
            serde_json::Value::String(s) if s.starts_with('$') => {
                let id = &s[1..];
                if !params.iter().any(|p| p.id == id) {
                    bail!("\"{s}\" names no slider of this effect");
                }
                Setting::Param(id.into())
            }
            _ => bail!(
                "The {kind_name} block's \"{name}\" must be a number or \"$slider\", the id of a slider with $ in front"
            ),
        };
        settings.insert(name, setting);
    }
    Ok(BlockSpec {
        kind,
        shape,
        settings,
    })
}

// ---------------------------------------------------------------- built-in blocks

#[derive(Clone, Copy, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}
impl Biquad {
    /// RBJ Audio EQ Cookbook.
    fn new(shape: Shape, rate: f64, frequency: f64, q: f64, gain_db: f64) -> Self {
        let frequency = frequency.clamp(10., rate * 0.49);
        let w = 2. * PI * frequency / rate;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2. * q.max(0.05));
        let a = 10f64.powf(gain_db / 40.);
        let (b0, b1, b2, a0, a1, a2) = match shape {
            Shape::Lowpass => (
                (1. - cos) / 2.,
                1. - cos,
                (1. - cos) / 2.,
                1. + alpha,
                -2. * cos,
                1. - alpha,
            ),
            Shape::Highpass => (
                (1. + cos) / 2.,
                -(1. + cos),
                (1. + cos) / 2.,
                1. + alpha,
                -2. * cos,
                1. - alpha,
            ),
            Shape::Bandpass => (alpha, 0., -alpha, 1. + alpha, -2. * cos, 1. - alpha),
            Shape::Notch => (1., -2. * cos, 1., 1. + alpha, -2. * cos, 1. - alpha),
            Shape::Peak => (
                1. + alpha * a,
                -2. * cos,
                1. - alpha * a,
                1. + alpha / a,
                -2. * cos,
                1. - alpha / a,
            ),
            Shape::Lowshelf | Shape::Highshelf => {
                let root = 2. * a.sqrt() * alpha;
                let sign = if shape == Shape::Lowshelf { 1. } else { -1. };
                (
                    a * ((a + 1.) - sign * (a - 1.) * cos + root),
                    sign * 2. * a * ((a - 1.) - sign * (a + 1.) * cos),
                    a * ((a + 1.) - sign * (a - 1.) * cos - root),
                    (a + 1.) + sign * (a - 1.) * cos + root,
                    -sign * 2. * ((a - 1.) + sign * (a + 1.) * cos),
                    (a + 1.) + sign * (a - 1.) * cos - root,
                )
            }
        };
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }
    fn run(&self, state: &mut [f64; 2], x: f64) -> f64 {
        let y = self.b0 * x + state[0];
        state[0] = self.b1 * x - self.a1 * y + state[1];
        state[1] = self.b2 * x - self.a2 * y;
        y
    }
}

fn db(value: f64) -> f64 {
    10f64.powf(value / 20.)
}
/// A one-pole smoothing coefficient for a time in milliseconds.
fn coefficient(ms: f64, rate: f64) -> f64 {
    (-1. / (ms.max(0.01) * 0.001 * rate)).exp()
}

/// A delay line long enough for `seconds`.
struct Line {
    data: Vec<f32>,
    write: usize,
}
impl Line {
    fn new(length: usize) -> Self {
        Self {
            data: vec![0.; length.max(2)],
            write: 0,
        }
    }
    fn push(&mut self, x: f32) {
        self.data[self.write] = x;
        self.write = (self.write + 1) % self.data.len();
    }
    /// The sample `delay` samples ago (fractional, linear interpolation).
    fn read(&self, delay: f64) -> f32 {
        let len = self.data.len();
        let delay = delay.clamp(1., (len - 1) as f64);
        let whole = delay.floor() as usize;
        let fraction = (delay - whole as f64) as f32;
        let at = |d: usize| self.data[(self.write + len - d) % len];
        at(whole) * (1. - fraction) + at((whole + 1).min(len - 1)) * fraction
    }
    fn clear(&mut self) {
        self.data.iter_mut().for_each(|s| *s = 0.);
    }
}

/// Freeverb's comb and all-pass filters.
struct Comb {
    buffer: Vec<f32>,
    index: usize,
    store: f32,
}
impl Comb {
    fn run(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let y = self.buffer[self.index];
        self.store = y * (1. - damp) + self.store * damp;
        self.buffer[self.index] = x + self.store * feedback;
        self.index = (self.index + 1) % self.buffer.len();
        y
    }
}
struct Allpass {
    buffer: Vec<f32>,
    index: usize,
}
impl Allpass {
    fn run(&mut self, x: f32) -> f32 {
        let delayed = self.buffer[self.index];
        let y = delayed - x;
        self.buffer[self.index] = x + delayed * 0.5;
        self.index = (self.index + 1) % self.buffer.len();
        y
    }
}
const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;

enum Block {
    Gain {
        factor: f32,
    },
    Filter {
        shape: Shape,
        filter: Biquad,
        state: Vec<[f64; 2]>,
    },
    Compressor {
        threshold: f64,
        ratio: f64,
        attack: f64,
        release: f64,
        makeup: f64,
        envelope: f64,
        limit: bool,
    },
    Delay {
        lines: Vec<Line>,
        samples: f64,
        feedback: f32,
        mix: f32,
    },
    Reverb {
        combs: Vec<Vec<Comb>>,
        allpasses: Vec<Vec<Allpass>>,
        feedback: f32,
        damp: f32,
        mix: f32,
        width: f32,
    },
    Chorus {
        lines: Vec<Line>,
        phase: f64,
        step: f64,
        base: f64,
        depth: f64,
        mix: f32,
    },
    Tremolo {
        phase: f64,
        step: f64,
        depth: f32,
    },
    Saturate {
        drive: f32,
        mix: f32,
    },
    Width {
        amount: f32,
    },
    Pan {
        left: f32,
        right: f32,
    },
}

impl Block {
    fn new(spec: &BlockSpec, rate: f64, channels: usize) -> Self {
        let lines = |seconds: f64| {
            (0..channels)
                .map(|_| Line::new((seconds * rate) as usize + 4))
                .collect()
        };
        let scale = rate / 44100.;
        match spec.kind {
            BlockKind::Gain => Self::Gain { factor: 1. },
            BlockKind::Filter => Self::Filter {
                shape: spec.shape.unwrap_or(Shape::Peak),
                filter: Biquad::default(),
                state: vec![[0.; 2]; channels],
            },
            BlockKind::Compressor | BlockKind::Limiter => Self::Compressor {
                threshold: 0.,
                ratio: 1.,
                attack: 0.,
                release: 0.,
                makeup: 1.,
                envelope: 0.,
                limit: spec.kind == BlockKind::Limiter,
            },
            BlockKind::Delay => Self::Delay {
                lines: lines(2.05),
                samples: 1.,
                feedback: 0.,
                mix: 0.,
            },
            BlockKind::Reverb => Self::Reverb {
                combs: (0..2)
                    .map(|side| {
                        COMBS
                            .iter()
                            .map(|n| Comb {
                                buffer: vec![0.; ((n + side * SPREAD) as f64 * scale) as usize + 1],
                                index: 0,
                                store: 0.,
                            })
                            .collect()
                    })
                    .collect(),
                allpasses: (0..2)
                    .map(|side| {
                        ALLPASSES
                            .iter()
                            .map(|n| Allpass {
                                buffer: vec![0.; ((n + side * SPREAD) as f64 * scale) as usize + 1],
                                index: 0,
                            })
                            .collect()
                    })
                    .collect(),
                feedback: 0.,
                damp: 0.,
                mix: 0.,
                width: 1.,
            },
            BlockKind::Chorus => Self::Chorus {
                lines: lines(0.05),
                phase: 0.,
                step: 0.,
                base: 0.007 * rate,
                depth: 0.,
                mix: 0.,
            },
            BlockKind::Tremolo => Self::Tremolo {
                phase: 0.,
                step: 0.,
                depth: 0.,
            },
            BlockKind::Saturate => Self::Saturate { drive: 1., mix: 1. },
            BlockKind::Width => Self::Width { amount: 1. },
            BlockKind::Pan => Self::Pan {
                left: 1.,
                right: 1.,
            },
        }
    }

    /// Apply settings without losing the sound already in delay lines and filters.
    fn configure(&mut self, get: &dyn Fn(&str) -> f64, rate: f64) {
        match self {
            Self::Gain { factor } => *factor = db(get("db")) as f32,
            Self::Filter { shape, filter, .. } => {
                *filter = Biquad::new(*shape, rate, get("frequency"), get("q"), get("gain"))
            }
            Self::Compressor {
                threshold,
                ratio,
                attack,
                release,
                makeup,
                limit,
                ..
            } => {
                if *limit {
                    *threshold = get("ceiling");
                    *ratio = f64::INFINITY;
                    *attack = 0.;
                    *makeup = 1.;
                } else {
                    *threshold = get("threshold");
                    *ratio = get("ratio");
                    *attack = coefficient(get("attack"), rate);
                    *makeup = db(get("makeup"));
                }
                *release = coefficient(get("release"), rate);
            }
            Self::Delay {
                samples,
                feedback,
                mix,
                ..
            } => {
                *samples = get("time") * 0.001 * rate;
                *feedback = get("feedback") as f32;
                *mix = get("mix") as f32;
            }
            Self::Reverb {
                feedback,
                damp,
                mix,
                width,
                ..
            } => {
                *feedback = (get("size") * 0.28 + 0.7) as f32;
                *damp = (get("damping") * 0.4) as f32;
                *mix = get("mix") as f32;
                *width = get("width") as f32;
            }
            Self::Chorus {
                step, depth, mix, ..
            } => {
                *step = 2. * PI * get("rate") / rate;
                *depth = get("depth") * 0.001 * rate;
                *mix = get("mix") as f32;
            }
            Self::Tremolo { step, depth, .. } => {
                *step = 2. * PI * get("rate") / rate;
                *depth = get("depth") as f32;
            }
            Self::Saturate { drive, mix } => {
                *drive = db(get("drive")) as f32;
                *mix = get("mix") as f32;
            }
            Self::Width { amount } => *amount = get("amount") as f32,
            Self::Pan { left, right } => {
                // Constant-power panning, normalised so the centre is unchanged.
                let angle = (get("position") + 1.) * PI / 4.;
                *left = (angle.cos() * std::f64::consts::SQRT_2) as f32;
                *right = (angle.sin() * std::f64::consts::SQRT_2) as f32;
            }
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Filter { state, .. } => state.iter_mut().for_each(|s| *s = [0.; 2]),
            Self::Compressor { envelope, .. } => *envelope = 0.,
            Self::Delay { lines, .. } | Self::Chorus { lines, .. } => {
                lines.iter_mut().for_each(Line::clear)
            }
            Self::Reverb {
                combs, allpasses, ..
            } => {
                for comb in combs.iter_mut().flatten() {
                    comb.buffer.iter_mut().for_each(|s| *s = 0.);
                    comb.store = 0.;
                }
                for allpass in allpasses.iter_mut().flatten() {
                    allpass.buffer.iter_mut().for_each(|s| *s = 0.);
                }
            }
            _ => {}
        }
    }

    fn process(&mut self, frame: &mut [f32]) {
        let channels = frame.len();
        match self {
            Self::Gain { factor } => frame.iter_mut().for_each(|s| *s *= *factor),
            Self::Filter { filter, state, .. } => {
                for (sample, state) in frame.iter_mut().zip(state.iter_mut()) {
                    *sample = filter.run(state, *sample as f64) as f32;
                }
            }
            Self::Compressor {
                threshold,
                ratio,
                attack,
                release,
                makeup,
                envelope,
                ..
            } => {
                // One detector for all channels keeps the stereo image steady.
                let peak = frame.iter().fold(0f64, |m, s| m.max((*s as f64).abs()));
                let coefficient = if peak > *envelope { *attack } else { *release };
                *envelope = peak + coefficient * (*envelope - peak);
                let level = 20. * envelope.max(1e-9).log10();
                let over = level - *threshold;
                let reduction = if over > 0. {
                    over * (1. - 1. / *ratio)
                } else {
                    0.
                };
                let gain = (db(-reduction) * *makeup) as f32;
                frame.iter_mut().for_each(|s| *s *= gain);
            }
            Self::Delay {
                lines,
                samples,
                feedback,
                mix,
            } => {
                for (sample, line) in frame.iter_mut().zip(lines.iter_mut()) {
                    let echo = line.read(*samples);
                    line.push(*sample + echo * *feedback);
                    *sample = *sample * (1. - *mix) + echo * *mix;
                }
            }
            Self::Reverb {
                combs,
                allpasses,
                feedback,
                damp,
                mix,
                width,
            } => {
                let input = frame.iter().sum::<f32>() / channels as f32 * 0.03;
                let mut wet = [0f32; 2];
                for side in 0..2 {
                    let mut out = 0.;
                    for comb in combs[side].iter_mut() {
                        out += comb.run(input, *feedback, *damp);
                    }
                    for allpass in allpasses[side].iter_mut() {
                        out = allpass.run(out);
                    }
                    wet[side] = out;
                }
                let wet1 = *width / 2. + 0.5;
                let wet2 = (1. - *width) / 2.;
                let (left, right) = (wet[0] * wet1 + wet[1] * wet2, wet[1] * wet1 + wet[0] * wet2);
                for (channel, sample) in frame.iter_mut().enumerate() {
                    let wet = match channel {
                        0 if channels == 1 => (left + right) * 0.5,
                        0 => left,
                        1 => right,
                        _ => (left + right) * 0.5,
                    };
                    *sample = *sample * (1. - *mix) + wet * *mix;
                }
            }
            Self::Chorus {
                lines,
                phase,
                step,
                base,
                depth,
                mix,
            } => {
                for (channel, (sample, line)) in frame.iter_mut().zip(lines.iter_mut()).enumerate()
                {
                    // The second channel's sweep runs a quarter turn later, for width.
                    let sweep = (*phase + channel as f64 * PI / 2.).sin();
                    let delay = *base + *depth * (1. + sweep) * 0.5;
                    line.push(*sample);
                    let voice = line.read(delay);
                    *sample = *sample * (1. - *mix * 0.5) + voice * *mix * 0.5;
                }
                *phase = (*phase + *step) % (2. * PI);
            }
            Self::Tremolo { phase, step, depth } => {
                let gain = 1. - *depth * 0.5 * (1. - phase.sin() as f32);
                frame.iter_mut().for_each(|s| *s *= gain);
                *phase = (*phase + *step) % (2. * PI);
            }
            Self::Saturate { drive, mix } => {
                let norm = drive.tanh().max(1e-6);
                for sample in frame.iter_mut() {
                    let shaped = (*sample * *drive).tanh() / norm;
                    *sample = *sample * (1. - *mix) + shaped * *mix;
                }
            }
            Self::Width { amount } => {
                if channels >= 2 {
                    let mid = (frame[0] + frame[1]) * 0.5;
                    let side = (frame[0] - frame[1]) * 0.5 * *amount;
                    frame[0] = mid + side;
                    frame[1] = mid - side;
                }
            }
            Self::Pan { left, right } => {
                if channels >= 2 {
                    frame[0] *= left.min(1.);
                    frame[1] *= right.min(1.);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- WebAssembly

struct Limits(wasmi::StoreLimits);

struct WasmRunner {
    store: wasmi::Store<Limits>,
    memory: wasmi::Memory,
    buffer: usize,
    process: wasmi::TypedFunc<i32, ()>,
    param: Option<wasmi::TypedFunc<(i32, f32), ()>>,
    reset: Option<wasmi::TypedFunc<(), ()>>,
    channels: usize,
    /// Blocks in a row that took longer than half their playing time.
    slow: u32,
}

impl WasmRunner {
    fn new(code: &WasmCode, rate: u32, channels: usize) -> Result<Self> {
        let limits = wasmi::StoreLimitsBuilder::new()
            .memory_size(WASM_MEMORY)
            .instances(1)
            .memories(1)
            .tables(1)
            .build();
        let mut store = wasmi::Store::new(&code.engine, Limits(limits));
        store.limiter(|data| &mut data.0);
        store
            .set_fuel(50_000_000)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let linker = wasmi::Linker::<Limits>::new(&code.engine);
        let instance = linker
            .instantiate_and_start(&mut store, &code.module)
            .map_err(|e| anyhow::anyhow!("Cannot start the module: {e}"))?;
        let memory = instance
            .get_memory(&store, "memory")
            .context("The module must export its memory as \"memory\"")?;
        let init = instance
            .get_typed_func::<(i32, i32, i32), i32>(&store, "init")
            .map_err(|_| {
                anyhow::anyhow!(
                    "The module must export init(rate: i32, channels: i32, max_frames: i32) -> i32"
                )
            })?;
        let process = instance
            .get_typed_func::<i32, ()>(&store, "process")
            .map_err(|_| anyhow::anyhow!("The module must export process(frames: i32)"))?;
        let param = instance
            .get_typed_func::<(i32, f32), ()>(&store, "param")
            .ok();
        let reset = instance.get_typed_func::<(), ()>(&store, "reset").ok();
        let buffer = init
            .call(
                &mut store,
                (rate as i32, channels as i32, BLOCK_FRAMES as i32),
            )
            .map_err(|e| anyhow::anyhow!("init failed: {e}"))?;
        let needed = BLOCK_FRAMES * channels * 4;
        if buffer < 0
            || buffer as u64 + needed as u64 > memory.data(&store).len() as u64
            || buffer % 4 != 0
        {
            bail!("init returned a buffer that does not fit in the module's memory");
        }
        Ok(Self {
            store,
            memory,
            buffer: buffer as usize,
            process,
            param,
            reset,
            channels,
            slow: 0,
        })
    }

    fn set_params(&mut self, values: &[f32]) -> Result<()> {
        if let Some(param) = self.param {
            for (index, value) in values.iter().enumerate() {
                self.store
                    .set_fuel(1_000_000)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                param
                    .call(&mut self.store, (index as i32, *value))
                    .map_err(|e| anyhow::anyhow!("param failed: {e}"))?;
            }
        }
        Ok(())
    }

    fn process(&mut self, block: &mut [f32], rate: u32) -> Result<()> {
        let frames = block.len() / self.channels;
        let start = Instant::now();
        let bytes = self.memory.data_mut(&mut self.store);
        let region = bytes
            .get_mut(self.buffer..self.buffer + block.len() * 4)
            .context("The module's buffer moved out of its memory")?;
        for (chunk, sample) in region.as_chunks_mut::<4>().0.iter_mut().zip(block.iter()) {
            chunk.copy_from_slice(&sample.to_le_bytes());
        }
        // Enough for heavy DSP; a loop that never ends runs out and stops.
        let fuel = block.len() as u64 * 4000 + 200_000;
        self.store
            .set_fuel(fuel)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.process
            .call(&mut self.store, frames as i32)
            .map_err(|e| {
                if self.store.get_fuel().is_ok_and(|f| f == 0) {
                    anyhow::anyhow!("It used too much work for one block of sound")
                } else {
                    anyhow::anyhow!("It stopped with an error: {e}")
                }
            })?;
        let bytes = self.memory.data(&self.store);
        let region = bytes
            .get(self.buffer..self.buffer + block.len() * 4)
            .context("The module's buffer moved out of its memory")?;
        for (sample, chunk) in block.iter_mut().zip(region.as_chunks::<4>().0) {
            let value = f32::from_le_bytes(*chunk);
            *sample = if value.is_finite() { value } else { 0. };
        }
        let playing_time = frames as f64 / rate.max(1) as f64;
        if start.elapsed().as_secs_f64() > playing_time * 0.5 {
            self.slow += 1;
            if self.slow >= 8 {
                bail!("It is too slow to keep up with the music");
            }
        } else {
            self.slow = 0;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- the rack

enum Engine {
    Blocks(Vec<(BlockSpec, Block)>),
    Wasm(Option<Box<WasmRunner>>),
}

struct Running {
    uid: String,
    def: Arc<EffectDef>,
    values: Vec<f32>,
    engine: Engine,
    /// The block before a WebAssembly effect ran, to play if it fails.
    backup: Vec<f32>,
}

impl Running {
    fn new(
        def: Arc<EffectDef>,
        values: Vec<f32>,
        rate: u32,
        channels: usize,
        registry: &Registry,
    ) -> Self {
        let engine = match &def.kind {
            EffectKind::Blocks(specs) => Engine::Blocks(
                specs
                    .iter()
                    .map(|spec| (spec.clone(), Block::new(spec, rate as f64, channels)))
                    .collect(),
            ),
            EffectKind::Wasm(code) => Engine::Wasm(match WasmRunner::new(code, rate, channels) {
                Ok(runner) => Some(Box::new(runner)),
                Err(error) => {
                    registry.fail(&def, format!("{error:#}"));
                    None
                }
            }),
        };
        let mut running = Self {
            uid: String::new(),
            def,
            values: vec![],
            engine,
            backup: Vec::with_capacity(BLOCK_FRAMES * channels),
        };
        running.configure(values, rate, registry);
        running
    }

    fn configure(&mut self, values: Vec<f32>, rate: u32, registry: &Registry) {
        match &mut self.engine {
            Engine::Blocks(blocks) => {
                for (spec, block) in blocks.iter_mut() {
                    let defaults = block_settings(spec.kind);
                    let get = |name: &str| -> f64 {
                        let (_, start, low, high) = defaults
                            .iter()
                            .find(|(k, ..)| *k == name)
                            .copied()
                            .unwrap_or((name, 0., f32::MIN, f32::MAX));
                        let value = match spec.settings.get(name) {
                            Some(Setting::Fixed(v)) => *v,
                            Some(Setting::Param(id)) => self
                                .def
                                .params
                                .iter()
                                .position(|p| p.id == *id)
                                .and_then(|i| values.get(i).copied())
                                .unwrap_or(start),
                            None => start,
                        };
                        value.clamp(low, high) as f64
                    };
                    block.configure(&get, rate as f64);
                }
            }
            Engine::Wasm(runner) => {
                if values != self.values
                    && let Some(active) = runner
                    && let Err(error) = active.set_params(&values)
                {
                    registry.fail(&self.def, format!("{error:#}"));
                    *runner = None;
                }
            }
        }
        self.values = values;
    }

    fn process(&mut self, block: &mut [f32], channels: usize, rate: u32, registry: &Registry) {
        match &mut self.engine {
            Engine::Blocks(blocks) => {
                for frame in block.chunks_exact_mut(channels) {
                    for (_, b) in blocks.iter_mut() {
                        b.process(frame);
                    }
                }
            }
            Engine::Wasm(runner) => {
                if let Some(active) = runner {
                    self.backup.clear();
                    self.backup.extend_from_slice(block);
                    if let Err(error) = active.process(block, rate) {
                        // Stop this effect for the rest of the song and play on without it.
                        block.copy_from_slice(&self.backup);
                        registry.fail(&self.def, format!("{error:#}"));
                        *runner = None;
                    }
                }
            }
        }
    }

    fn reset(&mut self, registry: &Registry) {
        match &mut self.engine {
            Engine::Blocks(blocks) => blocks.iter_mut().for_each(|(_, b)| b.reset()),
            Engine::Wasm(runner) => {
                if let Some(active) = runner
                    && let Some(reset) = active.reset
                {
                    let ok = active.store.set_fuel(1_000_000).is_ok()
                        && reset.call(&mut active.store, ()).is_ok();
                    if !ok {
                        registry.fail(&self.def, "reset failed".into());
                        *runner = None;
                    }
                }
            }
        }
    }
}

/// The effects running for one song.
pub struct Rack {
    rate: u32,
    channels: usize,
    running: Vec<Running>,
    registry: Arc<Registry>,
}

impl Rack {
    pub fn new(registry: Arc<Registry>, rate: u32, channels: u16) -> Self {
        Self {
            rate: rate.max(1),
            channels: channels.max(1) as usize,
            running: vec![],
            registry,
        }
    }
    pub fn registry(&self) -> &Arc<Registry> {
        &self.registry
    }
    pub fn is_empty(&self) -> bool {
        self.running.is_empty()
    }
    /// Match the listener's chain. Effects that stay keep their sound (echo tails, filter state).
    pub fn sync(&mut self, slots: &[EffectSlot]) {
        let mut old: Vec<Option<Running>> = std::mem::take(&mut self.running)
            .into_iter()
            .map(Some)
            .collect();
        for slot in slots.iter().filter(|s| s.on) {
            let Some(def) = self.registry.find(&slot.plugin, &slot.effect) else {
                continue;
            };
            let values = def.values(slot);
            let reused = old
                .iter_mut()
                .find_map(|r| r.take_if(|r| r.uid == slot.uid && Arc::ptr_eq(&r.def, &def)));
            let running = match reused {
                Some(mut running) => {
                    if running.values != values {
                        running.configure(values, self.rate, &self.registry);
                    }
                    running
                }
                None => {
                    let mut running =
                        Running::new(def, values, self.rate, self.channels, &self.registry);
                    running.uid = slot.uid.clone();
                    running
                }
            };
            self.running.push(running);
        }
    }
    /// Process interleaved frames in place; `block` holds whole frames only.
    pub fn process(&mut self, block: &mut [f32]) {
        for running in self.running.iter_mut() {
            running.process(block, self.channels, self.rate, &self.registry);
        }
    }
    /// Forget sound in delay lines and filters, after a seek.
    pub fn reset(&mut self) {
        for running in self.running.iter_mut() {
            running.reset(&self.registry);
        }
    }
}

/// Load a plugin's effects from what its `effects()` function returned.
pub fn load(
    plugin: &str,
    plugin_name: &str,
    folder: &Path,
    value: serde_json::Value,
) -> Result<Vec<EffectDef>> {
    parse(plugin, plugin_name, value, |name| {
        let path = Path::new(name);
        if name.is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            bail!("Use a plain file name inside the plugin's folder");
        }
        Ok(std::fs::read(folder.join(path))?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn registry_with(defs: Vec<EffectDef>) -> Arc<Registry> {
        let registry = Arc::new(Registry::default());
        registry.set(defs.into_iter().map(Arc::new).collect());
        registry
    }
    fn slot(effect: &str, params: &[(&str, f32)]) -> EffectSlot {
        EffectSlot {
            uid: format!("uid-{effect}"),
            plugin: "p".into(),
            effect: effect.into(),
            on: true,
            params: params.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        }
    }
    fn no_files(_: &str) -> Result<Vec<u8>> {
        bail!("no files")
    }
    fn sine(frequency: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let s = (2. * PI * frequency * i as f64 / 48000.).sin() as f32 * 0.5;
                [s, s]
            })
            .collect()
    }
    fn rms(samples: &[f32]) -> f64 {
        let tail = &samples[samples.len() / 2..];
        (tail.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / tail.len() as f64).sqrt()
    }
    fn run(rack: &mut Rack, input: &[f32]) -> Vec<f32> {
        let mut out = input.to_vec();
        for block in out.chunks_mut(BLOCK_FRAMES * 2) {
            rack.process(block);
        }
        out
    }

    #[test]
    fn block_effects_follow_their_sliders() {
        let defs = parse(
            "p",
            "P",
            json!([{
                "id": "dark", "name": "Dark",
                "params": [{ "id": "cut", "name": "Cut", "min": 200, "max": 20000, "value": 500, "unit": "Hz" }],
                "blocks": [
                    { "kind": "filter", "shape": "lowpass", "frequency": "$cut", "q": 0.707 },
                    { "kind": "gain", "db": 0 }
                ]
            }]),
            no_files,
        )
        .unwrap();
        let registry = registry_with(defs);
        let mut rack = Rack::new(registry, 48000, 2);
        rack.sync(&[slot("dark", &[])]);
        let high = sine(5000., 48000);
        let dark = rms(&run(&mut rack, &high)) / rms(&high);
        assert!(
            dark < 0.1,
            "a 500 Hz low-pass should remove 5 kHz, kept {dark}"
        );
        rack.sync(&[slot("dark", &[("cut", 20000.)])]);
        let open = rms(&run(&mut rack, &high)) / rms(&high);
        assert!(
            open > 0.9,
            "a 20 kHz low-pass should keep 5 kHz, kept {open}"
        );
        // Turned off, it does nothing.
        rack.sync(&[EffectSlot {
            on: false,
            ..slot("dark", &[])
        }]);
        assert!(rack.is_empty());
    }

    #[test]
    fn every_block_kind_runs_and_stays_finite() {
        let blocks: Vec<serde_json::Value> = [
            json!({ "kind": "gain", "db": -3 }),
            json!({ "kind": "filter", "shape": "highshelf", "frequency": 6000, "gain": 3 }),
            json!({ "kind": "filter", "shape": "lowshelf", "frequency": 100, "gain": -3 }),
            json!({ "kind": "filter", "shape": "notch", "frequency": 60 }),
            json!({ "kind": "filter", "shape": "bandpass", "frequency": 1000, "q": 0.5 }),
            json!({ "kind": "filter", "shape": "highpass", "frequency": 20 }),
            json!({ "kind": "filter", "shape": "peak", "frequency": 1000, "gain": 2 }),
            json!({ "kind": "compressor" }),
            json!({ "kind": "saturate", "drive": 12 }),
            json!({ "kind": "chorus" }),
            json!({ "kind": "delay", "time": 120, "feedback": 0.5 }),
            json!({ "kind": "reverb", "size": 0.9 }),
            json!({ "kind": "tremolo" }),
            json!({ "kind": "width", "amount": 1.5 }),
            json!({ "kind": "pan", "position": 0.3 }),
            json!({ "kind": "limiter", "ceiling": -1 }),
        ]
        .into();
        let defs = parse(
            "p",
            "P",
            json!([{ "id": "all", "name": "All", "blocks": blocks }]),
            no_files,
        )
        .unwrap();
        let mut rack = Rack::new(registry_with(defs), 48000, 2);
        rack.sync(&[slot("all", &[])]);
        let out = run(&mut rack, &sine(440., 48000));
        assert!(out.iter().all(|s| s.is_finite()));
        let peak = out[out.len() / 2..]
            .iter()
            .fold(0f32, |m, s| m.max(s.abs()));
        assert!(peak > 0.01, "sound should come through");
        assert!(
            peak < 0.95,
            "the limiter should hold the peak near -1 dB, got {peak}"
        );
        // An echo keeps ringing after the input stops.
        let defs = parse("p", "P", json!([{ "id": "echo", "name": "Echo", "blocks": [{ "kind": "delay", "time": 10, "mix": 0.5 }] }]), no_files).unwrap();
        let mut rack = Rack::new(registry_with(defs), 48000, 2);
        rack.sync(&[slot("echo", &[])]);
        let mut input = vec![0.5f32; 2 * 100];
        input.extend(vec![0f32; 2 * 1000]);
        let out = run(&mut rack, &input);
        assert!(out[2 * 500] != 0.);
        rack.reset();
        let silent = run(&mut rack, &vec![0f32; 2 * 1000]);
        assert!(silent.iter().all(|s| *s == 0.));
    }

    #[test]
    fn bad_definitions_are_explained() {
        let error = |value: serde_json::Value| {
            format!("{:#}", parse("p", "P", value, no_files).unwrap_err())
        };
        assert!(error(json!([{ "id": "x", "name": "X" }])).contains("needs blocks or a wasm file"));
        assert!(
            error(json!([{ "id": "x", "name": "X", "blocks": [{ "kind": "wobble" }] }]))
                .contains("Unknown block kind")
        );
        assert!(
            error(json!([{ "id": "x", "name": "X", "blocks": [{ "kind": "filter" }] }]))
                .contains("need a shape")
        );
        assert!(
            error(json!([{ "id": "x", "name": "X", "blocks": [{ "kind": "gain", "volume": 1 }] }]))
                .contains("It has: db")
        );
        assert!(
            error(
                json!([{ "id": "x", "name": "X", "blocks": [{ "kind": "gain", "db": "$nope" }] }])
            )
            .contains("names no slider")
        );
        assert!(
            error(json!([{ "id": "x y", "name": "X", "blocks": [{ "kind": "gain" }] }]))
                .contains("letters, digits")
        );
        assert!(
            error(json!([{ "id": "x", "name": "X", "wasm": "../up.wasm" }]))
                .contains("cannot read")
        );
    }

    const GAIN_WAT: &str = r#"
        (module
          (memory (export "memory") 1)
          (global $gain (mut f32) (f32.const 1))
          (func (export "init") (param i32 i32 i32) (result i32) (i32.const 1024))
          (func (export "param") (param $i i32) (param $v f32)
            (if (i32.eqz (local.get $i)) (then (global.set $gain (local.get $v)))))
          (func (export "process") (param $frames i32)
            (local $p i32) (local $end i32)
            (local.set $p (i32.const 1024))
            ;; two channels, four bytes a sample
            (local.set $end (i32.add (i32.const 1024) (i32.shl (local.get $frames) (i32.const 3))))
            (block $done (loop $next
              (br_if $done (i32.ge_u (local.get $p) (local.get $end)))
              (f32.store (local.get $p) (f32.mul (f32.load (local.get $p)) (global.get $gain)))
              (local.set $p (i32.add (local.get $p) (i32.const 4)))
              (br $next))))
        )"#;

    fn wasm_effect(wat: &str) -> Result<Vec<EffectDef>> {
        let bytes = wat::parse_str(wat).unwrap();
        parse(
            "p",
            "P",
            json!([{ "id": "w", "name": "W", "wasm": "w.wasm",
                     "params": [{ "id": "gain", "name": "Gain", "min": 0, "max": 2, "value": 0.5 }] }]),
            move |_| Ok(bytes.clone()),
        )
    }

    #[test]
    fn webassembly_effects_process_blocks_and_take_slider_values() {
        let registry = registry_with(wasm_effect(GAIN_WAT).unwrap());
        let mut rack = Rack::new(registry.clone(), 48000, 2);
        rack.sync(&[slot("w", &[])]);
        let out = run(&mut rack, &vec![0.8f32; 2 * 1500]);
        assert!(
            out.iter().all(|s| (*s - 0.4).abs() < 1e-6),
            "{:?}",
            &out[..4]
        );
        rack.sync(&[slot("w", &[("gain", 0.25)])]);
        let out = run(&mut rack, &[0.8f32; 2 * 10]);
        assert!(out.iter().all(|s| (*s - 0.2).abs() < 1e-6));
        assert!(registry.failure("p", "w").is_none());
    }

    #[test]
    fn the_example_on_the_plugins_page_works() {
        let page = include_str!("../../../website/public/plugins.html");
        let start = page
            .find("(module\n  (memory (export \"memory\") 1)\n  (func (export \"init\")")
            .unwrap();
        let end = start + page[start..].find("</code></pre>").unwrap();
        let registry = registry_with(wasm_effect(&page[start..end]).unwrap());
        let mut rack = Rack::new(registry, 48000, 2);
        rack.sync(&[slot("w", &[])]);
        let out = run(&mut rack, &[0.8f32; 2 * 700]);
        assert!(out.iter().all(|s| (*s - 0.4).abs() < 1e-6));
    }

    #[test]
    fn webassembly_that_misbehaves_is_stopped_and_the_music_goes_on() {
        let forever = r#"(module (memory (export "memory") 1)
            (func (export "init") (param i32 i32 i32) (result i32) (i32.const 0))
            (func (export "process") (param i32) (loop $l (br $l))))"#;
        let registry = registry_with(wasm_effect(forever).unwrap());
        let mut rack = Rack::new(registry.clone(), 48000, 2);
        rack.sync(&[slot("w", &[])]);
        let input = vec![0.3f32; 2 * 600];
        assert_eq!(run(&mut rack, &input), input);
        assert!(
            registry
                .failure("p", "w")
                .unwrap()
                .contains("too much work")
        );
        assert_eq!(run(&mut rack, &input), input);

        let imports = r#"(module (import "env" "open" (func)) (memory (export "memory") 1))"#;
        assert!(format!("{:#}", wasm_effect(imports).unwrap_err()).contains("cannot import"));
        let missing = r#"(module (memory (export "memory") 1))"#;
        assert!(format!("{:#}", wasm_effect(missing).unwrap_err()).contains("export init"));
        let outside = r#"(module (memory (export "memory") 1)
            (func (export "init") (param i32 i32 i32) (result i32) (i32.const 65000))
            (func (export "process") (param i32)))"#;
        assert!(format!("{:#}", wasm_effect(outside).unwrap_err()).contains("does not fit"));
    }
}
