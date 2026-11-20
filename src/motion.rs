// CubiCraft — motion & controllers.
use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

// motion3.json-style clips (keyed curves, eased segments, loop, fade),
// eye-blink, breath, expressions (exp3-style additive sets) and mouse look-at.

use crate::params::Params;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Key {
    pub t: f32,
    pub v: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Curve {
    pub pid: usize,
    pub keys: Vec<Key>,
    /// Add = additive on top of current value, else overwrite (weight-faded)
    pub add: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Clip {
    pub name: String,
    pub dur: f32,
    pub looped: bool,
    pub curves: Vec<Curve>,
}

fn sample_keys(keys: &[Key], t: f32) -> f32 {
    if keys.is_empty() {
        return 0.0;
    }
    if t <= keys[0].t {
        return keys[0].v;
    }
    if t >= keys[keys.len() - 1].t {
        return keys[keys.len() - 1].v;
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if t >= a.t && t <= b.t {
            let tt = ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0);
            let s = tt * tt * (3.0 - 2.0 * tt); // smoothstep easing
            return a.v + (b.v - a.v) * s;
        }
    }
    keys[keys.len() - 1].v
}

impl Clip {
    pub fn sample(&self, t: f32, params: &mut Params, weight: f32) {
        let t = if self.looped { t % self.dur } else { t.min(self.dur) };
        for c in &self.curves {
            let v = sample_keys(&c.keys, t);
            if c.add {
                params.add(c.pid, v * weight);
            } else {
                let base = params.get(c.pid);
                params.set(c.pid, base + (v - base) * weight);
            }
        }
    }
}

pub struct MotionPlayer {
    pub clip: Option<Clip>,
    pub t: f32,
    pub weight: f32,
    pub fading_out: bool,
}

impl MotionPlayer {
    pub fn new() -> Self {
        MotionPlayer { clip: None, t: 0.0, weight: 0.0, fading_out: false }
    }

    pub fn play(&mut self, clip: Clip) {
        if let Some(cur) = self.clip.take() {
            // quick fade-out of the previous one handled by replacing instantly
            let _ = cur;
        }
        self.clip = Some(clip);
        self.t = 0.0;
        self.weight = 0.0;
        self.fading_out = false;
    }

    pub fn stop(&mut self) {
        self.fading_out = true;
    }

    pub fn update(&mut self, params: &mut Params, dt: f32) -> bool {
        let Some(clip) = &self.clip else { return false };
        self.t += dt;
        if self.fading_out || (!clip.looped && self.t > clip.dur) {
            self.weight -= dt * 4.0;
            if self.weight <= 0.0 {
                self.weight = 0.0;
                self.clip = None;
                self.fading_out = false;
                return false;
            }
        } else {
            self.weight = (self.weight + dt * 6.0).min(1.0);
        }
        let w = self.weight;
        let t = self.t;
        clip.sample(t, params, w);
        true
    }

    pub fn playing(&self) -> bool {
        self.clip.is_some()
    }
}

pub struct Blink {
    pub enabled: bool,
    next_at: f32,
    t: f32,
    active: bool,
    pending_double: bool,
}

impl Blink {
    pub fn new() -> Self {
        Blink { enabled: true, next_at: 1.5, t: 0.0, active: false, pending_double: false }
    }

    fn curve(&self, t: f32) -> f32 {
        // 0 -> 1 (close) over 0.10s, hold 0.04s, open over 0.16s
        if t < 0.10 {
            let x = t / 0.10;
            x * x * (3.0 - 2.0 * x)
        } else if t < 0.14 {
            1.0
        } else if t < 0.30 {
            let x = (t - 0.14) / 0.16;
            1.0 - x * x * (3.0 - 2.0 * x)
        } else {
            0.0
        }
    }

    pub fn update(&mut self, params: &mut Params, time: f64, dt: f32) {
        if !self.enabled {
            self.active = false;
            return;
        }
        if !self.active && time as f32 >= self.next_at {
            self.active = true;
            self.t = 0.0;
        }
        if self.active {
            self.t += dt;
            let b = self.curve(self.t);
            for pid_name in ["ParamEyeLOpen", "ParamEyeROpen"] {
                let pid = params.pid(pid_name);
                let v = params.get(pid);
                params.set(pid, v * (1.0 - b));
            }
            if self.t >= 0.30 {
                self.active = false;
                let delay = if self.pending_double {
                    self.pending_double = false;
                    0.35
                } else {
                    if macroquad::rand::gen_range(0.0, 1.0) < 0.12 {
                        self.pending_double = true;
                        0.4
                    } else {
                        0.0
                    }
                };
                self.next_at = time as f32 + delay + macroquad::rand::gen_range(2.0, 5.5);
            }
        }
    }
}

pub struct Breath {
    pub t: f32,
}

impl Breath {
    pub fn new() -> Self {
        Breath { t: 0.0 }
    }
    pub fn update(&mut self, params: &mut Params, dt: f32) {
        self.t += dt;
        let v = 0.5 - 0.5 * (std::f32::consts::TAU * self.t / 3.6).cos();
        let pid = params.pid("ParamBreath");
        params.set(pid, v);
    }
}

pub struct Expression {
    pub sets: Vec<(&'static str, Vec<(&'static str, f32)>)>,
    pub current: usize,
    pub weight: f32,
}

impl Expression {
    pub fn new() -> Self {
        Expression {
            sets: vec![
                ("Neutral", vec![]),
                ("Smile", vec![
                    ("ParamMouthForm", 0.75),
                    ("ParamEyeLSmile", 0.8),
                    ("ParamEyeRSmile", 0.8),
                    ("ParamBrowLY", 0.25),
                    ("ParamBrowRY", 0.25),
                    ("ParamCheek", 0.75),
                ]),
                ("Angry", vec![
                    ("ParamMouthForm", -0.55),
                    ("ParamBrowLAngle", -0.7),
                    ("ParamBrowRAngle", -0.7),
                    ("ParamBrowLY", -0.1),
                    ("ParamBrowRY", -0.1),
                    ("ParamCheek", 0.25),
                ]),
                ("Surprised", vec![
                    ("ParamBrowLY", 0.6),
                    ("ParamBrowRY", 0.6),
                    ("ParamMouthOpenY", 0.3),
                    ("ParamAngleY", 4.0),
                    ("ParamCheek", 0.3),
                ]),
            ],
            current: 0,
            weight: 1.0,
        }
    }

    pub fn set(&mut self, idx: usize) {
        self.current = idx;
        self.weight = 0.0;
    }

    pub fn update(&mut self, params: &mut Params, dt: f32) {
        self.weight = (self.weight + dt * 4.0).min(1.0);
        for (name, v) in &self.sets[self.current].1 {
            let pid = params.pid(name);
            params.add(pid, *v * self.weight);
        }
    }
}

pub struct LookAt {
    pub enabled: bool,
    pub target: Vec2,
    pub smooth: Vec2,
}

impl LookAt {
    pub fn new() -> Self {
        LookAt { enabled: true, target: Vec2::ZERO, smooth: Vec2::ZERO }
    }

    pub fn update(&mut self, params: &mut Params, dt: f32) {
        if !self.enabled {
            self.target = Vec2::ZERO;
        }
        let k = 1.0 - (-dt * 9.0).exp();
        self.smooth += (self.target - self.smooth) * k;
        let dx = self.smooth.x;
        let dy = self.smooth.y;
        let set = |params: &mut Params, name: &str, v: f32| {
            let pid = params.pid(name);
            params.set(pid, v);
        };
        set(params, "ParamAngleX", dx * 30.0);
        set(params, "ParamAngleY", -dy * 30.0);
        set(params, "ParamAngleZ", dx * 9.0);
        set(params, "ParamEyeBallX", dx);
        set(params, "ParamEyeBallY", -dy);
    }
}
