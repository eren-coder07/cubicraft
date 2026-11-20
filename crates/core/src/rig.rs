//! Rig specification (JSON) + parameter registry.

use crate::math::clamp;
use serde::Deserialize;
use std::collections::HashMap;

/// Physical parameters of the hair spring-damper simulation.
#[derive(Debug, Clone, Deserialize)]
pub struct PhysSpec {
    #[serde(default = "d_stiff")]
    pub stiffness: f32,
    #[serde(default = "d_damp")]
    pub damping: f32,
    /// Pixel amplitude of the main sway at the hair tip.
    #[serde(default = "d_amp")]
    pub amp: f32,
    /// Pixel amplitude of the secondary (livelier) sway.
    #[serde(default = "d_amp2")]
    pub amp2: f32,
}
fn d_stiff() -> f32 {
    46.0
}
fn d_damp() -> f32 {
    7.0
}
fn d_amp() -> f32 {
    15.0
}
fn d_amp2() -> f32 {
    9.0
}
impl Default for PhysSpec {
    fn default() -> Self {
        Self { stiffness: d_stiff(), damping: d_damp(), amp: d_amp(), amp2: d_amp2() }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PartSpec {
    pub name: String,
    pub kind: String,
    /// Layer names composited bottom-to-top for this part's texture.
    #[serde(default)]
    pub layers: Vec<String>,
    /// Take texture by cropping an already-built part (for cutouts).
    #[serde(default)]
    pub from: String,
    /// Crop rect in the source part's texture-local coordinates: [x0,y0,x1,y1].
    #[serde(default)]
    pub rect: [f32; 4],
    /// Pivot in texture-local coords when `from` is set, canvas coords otherwise.
    #[serde(default)]
    pub pivot: [f32; 2],
    #[serde(default)]
    pub depth: f32,
    #[serde(default = "d_grid")]
    pub grid: [usize; 2],
    #[serde(default)]
    pub side: String,
    /// Procedural ellipse parts (mouth opening): radii + RGBA color.
    #[serde(default)]
    pub rx: f32,
    #[serde(default)]
    pub ry: f32,
    #[serde(default)]
    pub color: Option<[u8; 4]>,
}
fn d_grid() -> [usize; 2] {
    [4, 4]
}

#[derive(Debug, Clone, Deserialize)]
pub struct Spec {
    pub canvas: [f32; 2],
    pub neck_pivot: [f32; 2],
    pub hip_pivot: [f32; 2],
    #[serde(default)]
    pub physics: PhysSpec,
    pub parts: Vec<PartSpec>,
}

pub fn parse_spec(json: &str) -> Result<Spec, String> {
    serde_json::from_str(json).map_err(|e| format!("rig spec: {e}"))
}

/// A single animatable parameter with clamping range.
#[derive(Debug, Clone)]
pub struct ParamDef {
    pub id: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// The standard Cubism-inspired parameter set used by the engine.
pub const PARAM_DEFS: &[ParamDef] = &[
    ParamDef { id: "AngleX", min: -30.0, max: 30.0, default: 0.0 },
    ParamDef { id: "AngleY", min: -30.0, max: 30.0, default: 0.0 },
    ParamDef { id: "AngleZ", min: -30.0, max: 30.0, default: 0.0 },
    ParamDef { id: "EyeLOpen", min: 0.0, max: 1.0, default: 1.0 },
    ParamDef { id: "EyeROpen", min: 0.0, max: 1.0, default: 1.0 },
    ParamDef { id: "EyeBallX", min: -1.0, max: 1.0, default: 0.0 },
    ParamDef { id: "EyeBallY", min: -1.0, max: 1.0, default: 0.0 },
    ParamDef { id: "MouthOpenY", min: 0.0, max: 1.0, default: 0.0 },
    ParamDef { id: "BodyAngleX", min: -10.0, max: 10.0, default: 0.0 },
    ParamDef { id: "BodyAngleY", min: -10.0, max: 10.0, default: 0.0 },
    ParamDef { id: "BodyAngleZ", min: -10.0, max: 10.0, default: 0.0 },
    ParamDef { id: "DragX", min: -120.0, max: 120.0, default: 0.0 },
    ParamDef { id: "DragY", min: -120.0, max: 120.0, default: 0.0 },
    ParamDef { id: "HairSway", min: -40.0, max: 40.0, default: 0.0 },
    ParamDef { id: "HairSway2", min: -40.0, max: 40.0, default: 0.0 },
    ParamDef { id: "BrowUp", min: -1.0, max: 1.0, default: 0.0 },
];

#[derive(Debug, Clone, Default)]
pub struct Params {
    vals: HashMap<&'static str, f32>,
}

impl Params {
    pub fn new() -> Self {
        let mut p = Self::default();
        for d in PARAM_DEFS {
            p.vals.insert(d.id, d.default);
        }
        p
    }

    pub fn set(&mut self, id: &str, v: f32) {
        if let Some(d) = PARAM_DEFS.iter().find(|d| d.id == id) {
            let v = clamp(v, d.min, d.max);
            self.vals.insert(d.id, v);
        }
    }

    pub fn get(&self, id: &str) -> f32 {
        self.vals.get(id).copied().unwrap_or(0.0)
    }

    pub fn reset(&mut self) {
        for d in PARAM_DEFS {
            self.vals.insert(d.id, d.default);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_clamp() {
        let mut p = Params::new();
        p.set("AngleX", 999.0);
        assert_eq!(p.get("AngleX"), 30.0);
        p.set("AngleX", -999.0);
        assert_eq!(p.get("AngleX"), -30.0);
        p.set("EyeLOpen", 0.2);
        assert_eq!(p.get("EyeLOpen"), 0.2);
    }

    #[test]
    fn spec_parses() {
        let spec = parse_spec(include_str!("../../../assets/rig.json")).unwrap();
        assert_eq!(spec.canvas, [1078.0, 2559.0]);
        assert!(spec.parts.len() >= 8);
    }
}
