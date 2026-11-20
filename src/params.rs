// CubiCraft — parameters module.
// Mirrors the Cubism "standard parameter list" concept: named parameters with
// min/default/max, grouped, with Cubism-style asymmetric normalization.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: String,
    pub name: String,
    pub group: String,
    pub min: f32,
    pub def: f32,
    pub max: f32,
    /// driven by physics output
    pub physics: bool,
}

#[allow(clippy::too_many_arguments)]
fn p(id: &'static str, name: &'static str, group: &'static str, min: f32, def: f32, max: f32, physics: bool) -> ParamDef {
    ParamDef { id: id.into(), name: name.into(), group: group.into(), min, def, max, physics }
}

pub const GROUPS: &[&str] = &["Head", "Eyes", "Brows", "Mouth", "Body", "Hair", "Overall", "Custom"];

pub fn base_param_defs() -> Vec<ParamDef> {
    vec![
    p("ParamAngleX", "Angle X", "Head", -30.0, 0.0, 30.0, false),
    p("ParamAngleY", "Angle Y", "Head", -30.0, 0.0, 30.0, false),
    p("ParamAngleZ", "Angle Z", "Head", -30.0, 0.0, 30.0, false),
    p("ParamEyeLOpen", "Eye L Open", "Eyes", 0.0, 1.0, 1.0, false),
    p("ParamEyeROpen", "Eye R Open", "Eyes", 0.0, 1.0, 1.0, false),
    p("ParamEyeLSmile", "Eye L Smile", "Eyes", 0.0, 0.0, 1.0, false),
    p("ParamEyeRSmile", "Eye R Smile", "Eyes", 0.0, 0.0, 1.0, false),
    p("ParamEyeBallX", "Eyeball X", "Eyes", -1.0, 0.0, 1.0, false),
    p("ParamEyeBallY", "Eyeball Y", "Eyes", -1.0, 0.0, 1.0, false),
    p("ParamBrowLY", "Brow L Y", "Brows", -1.0, 0.0, 1.0, false),
    p("ParamBrowRY", "Brow R Y", "Brows", -1.0, 0.0, 1.0, false),
    p("ParamBrowLAngle", "Brow L Angle", "Brows", -1.0, 0.0, 1.0, false),
    p("ParamBrowRAngle", "Brow R Angle", "Brows", -1.0, 0.0, 1.0, false),
    p("ParamMouthForm", "Mouth Form", "Mouth", -1.0, 0.0, 1.0, false),
    p("ParamMouthOpenY", "Mouth Open", "Mouth", 0.0, 0.0, 1.0, false),
    p("ParamCheek", "Cheek", "Mouth", 0.0, 0.55, 1.0, false),
    p("ParamBodyAngleX", "Body Angle X", "Body", -10.0, 0.0, 10.0, false),
    p("ParamBodyAngleY", "Body Angle Y", "Body", -10.0, 0.0, 10.0, false),
    p("ParamBodyAngleZ", "Body Angle Z", "Body", -10.0, 0.0, 10.0, false),
    p("ParamBreath", "Breath", "Body", 0.0, 0.0, 1.0, false),
    p("ParamShoulderY", "Shoulder Y", "Body", -10.0, 0.0, 10.0, false),
    p("ParamArmLA", "Left Arm A", "Body", -30.0, 0.0, 30.0, false),
    p("ParamArmRA", "Right Arm A", "Body", -30.0, 0.0, 30.0, false),
    p("ParamHairFront", "Hair Front", "Hair", -1.0, 0.0, 1.0, true),
    p("ParamHairSide", "Hair Side", "Hair", -1.0, 0.0, 1.0, true),
    p("ParamHairBack", "Hair Back", "Hair", -1.0, 0.0, 1.0, true),
    p("ParamHairFluffy", "Hair Fluffy", "Hair", -1.0, 0.0, 1.0, true),
        p("ParamBaseX", "Base X", "Overall", -10.0, 0.0, 10.0, false),
        p("ParamBaseY", "Base Y", "Overall", -10.0, 0.0, 10.0, false),
    ]
}

pub struct Params {
    pub defs: Vec<ParamDef>,
    pub values: Vec<f32>,
    pub defaults: Vec<f32>,
    idx: HashMap<String, usize>,
    /// sticky manual overrides set by dragging sliders (param -> value)
    pub manual: HashMap<usize, f32>,
}

impl Params {
    pub fn new() -> Self {
        let defs = base_param_defs();
        let values = defs.iter().map(|d| d.def).collect();
        let defaults = defs.iter().map(|d| d.def).collect();
        let idx = defs.iter().enumerate().map(|(i, d)| (d.id.clone(), i)).collect();
        Params { defs, values, defaults, idx, manual: HashMap::new() }
    }

    pub fn pid(&self, id: &str) -> usize {
        *self.idx.get(id).unwrap_or_else(|| panic!("unknown param id: {id}"))
    }

    pub fn find(&self, id: &str) -> Option<usize> {
        self.idx.get(id).copied()
    }

    /// Drop all custom parameters, restoring the built-in set.
    pub fn reset_to_base(&mut self) {
        self.defs.truncate(crate::project::BASE_PARAM_COUNT);
        self.values.truncate(crate::project::BASE_PARAM_COUNT);
        self.defaults.truncate(crate::project::BASE_PARAM_COUNT);
        self.manual.clear();
        self.idx = self.defs.iter().enumerate().map(|(i, d)| (d.id.clone(), i)).collect();
    }

    /// Register a new custom parameter; returns its pid.
    pub fn add_param(&mut self, id: String, name: String, group: String, min: f32, def: f32, max: f32) -> usize {
        if let Some(existing) = self.find(&id) {
            return existing;
        }
        let pid = self.defs.len();
        self.defs.push(ParamDef { id: id.clone(), name, group, min, def, max, physics: false });
        self.values.push(def);
        self.defaults.push(def);
        self.idx.insert(id, pid);
        pid
    }

    #[inline]
    pub fn get(&self, pid: usize) -> f32 {
        self.values[pid]
    }

    #[inline]
    pub fn get_by(&self, id: &str) -> f32 {
        self.values[self.pid(id)]
    }

    /// Raw write (used by controllers with overwrite blending).
    #[inline]
    pub fn set(&mut self, pid: usize, v: f32) {
        let d = &self.defs[pid];
        self.values[pid] = v.clamp(d.min, d.max);
    }

    /// Additive write (Cubism "Add" blend).
    #[inline]
    pub fn add(&mut self, pid: usize, delta: f32) {
        let d = &self.defs[pid];
        self.values[pid] = (self.values[pid] + delta).clamp(d.min, d.max);
    }

    /// Cubism-style asymmetric normalization around the default value:
    /// 0 at default, -1..1 across the (default..min)/(default..max) spans.
    #[inline]
    pub fn norm(&self, pid: usize) -> f32 {
        let d = &self.defs[pid];
        let v = self.values[pid];
        if v >= d.def {
            if d.max - d.def <= 1e-6 {
                0.0
            } else {
                (v - d.def) / (d.max - d.def)
            }
        } else if d.def - d.min <= 1e-6 {
            0.0
        } else {
            (v - d.def) / (d.def - d.min)
        }
    }

    /// Reset to defaults, then re-apply manual sticky overrides.
    pub fn begin_frame(&mut self) {
        for (i, d) in self.defs.iter().enumerate() {
            self.values[i] = d.def;
        }
    }

    pub fn apply_manual(&mut self) {
        let entries: Vec<(usize, f32)> = self.manual.iter().map(|(k, v)| (*k, *v)).collect();
        for (pid, v) in entries {
            self.set(pid, v);
        }
    }
}
