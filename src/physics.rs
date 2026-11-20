// CubiCraft — physics.
use macroquad::prelude::*;
// Clean-room take on the Cubism physics model: each "physics setting" has
// pendulum strands (spring + inertial mass + damping) whose roots are kicked
// by input parameters (head/body angles); joint angles map back to output
// parameters (hair sway), like .physics3.json Pendulum/Input/Output entries.

use serde::{Deserialize, Serialize};

use crate::params::Params;
use crate::serde_math::v2;

#[derive(Clone, Serialize, Deserialize)]
pub struct InputDef {
    pub pid: usize,
    /// degrees contributed when the param is at its extreme (norm = 1)
    pub deg: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StrandDef {
    /// anchor point (model space) and rest direction (degrees, y-down screen space)
    #[serde(with = "v2")]
    pub anchor: Vec2,
    pub base_angle_deg: f32,
    pub n_joints: usize,
    pub rest_len: f32,
    pub stiffness: f32,
    pub inertial_mass: f32,
    pub damping: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OutputDef {
    pub strand: usize,
    pub joint: usize,
    pub pid: usize,
    /// output = clamp(rel_deg / 45, -1, 1) * scale
    pub scale: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicsSetting {
    pub name: String,
    pub inputs: Vec<InputDef>,
    pub strands: Vec<StrandDef>,
    pub outputs: Vec<OutputDef>,
}

pub struct Joint {
    pub pos: Vec2,
    pub prev: Vec2,
}

pub struct Strand {
    pub joints: Vec<Joint>,
}

pub struct Physics {
    pub settings: Vec<PhysicsSetting>,
    strands: Vec<Vec<Joint>>,
}

impl Physics {
    pub fn new(settings: Vec<PhysicsSetting>) -> Self {
        let mut strands = Vec::new();
        for s in &settings {
            for sd in &s.strands {
                let mut joints = Vec::new();
                for i in 0..=sd.n_joints {
                    let a = (sd.base_angle_deg).to_radians();
                    let d = sd.anchor + Vec2::new(a.cos(), a.sin()) * (sd.rest_len * i as f32);
                    joints.push(Joint { pos: d, prev: d });
                }
                strands.push(joints);
            }
        }
        Physics { settings, strands }
    }

    pub fn update(&mut self, params: &Params, dt: f32) {
        let dt = dt.clamp(1.0 / 240.0, 1.0 / 20.0);
        let mut strand_idx = 0usize;
        for s in &self.settings {
            // input angle (degrees) from weighted parameter contributions
            let mut input_deg = 0.0;
            for inp in &s.inputs {
                input_deg += params.norm(inp.pid) * inp.deg;
            }
            for sd in &s.strands {
                let joints = &mut self.strands[strand_idx];
                strand_idx += 1;
                // root direction = base + input
                let dir = (sd.base_angle_deg + input_deg).to_radians();
                joints[0].pos = sd.anchor;
                joints[0].prev = sd.anchor;
                // first joint is placed rigidly at rest length along root dir
                joints[1].pos = sd.anchor + Vec2::new(dir.cos(), dir.sin()) * sd.rest_len;
                joints[1].prev = joints[1].pos;
                let g = 3.2; // gravity strength (model units/s^2, y-down)
                for i in 2..joints.len() {
                    let parent = joints[i - 1].pos;
                    let p = joints[i].pos;
                    let prev = joints[i].prev;
                    // spring toward the rest pose direction (stiffness), gravity, damping
                    let vel = (p - prev) / dt;
                    let rest_dir = Vec2::new(dir.cos(), dir.sin());
                    let rest_pos = parent + rest_dir * sd.rest_len;
                    let force = (rest_pos - p) * sd.stiffness + Vec2::new(0.0, g) * sd.stiffness * 0.35;
                    let accel = force / sd.inertial_mass - vel * sd.damping;
                    let mut np = p + vel * dt + 0.5 * accel * dt * dt;
                    // constrain to rest length around parent
                    let mut dv = np - parent;
                    let l = dv.length().max(1e-6);
                    dv = dv / l * sd.rest_len;
                    np = parent + dv;
                    joints[i].prev = p;
                    joints[i].pos = np;
                }
            }
        }
    }

    /// Map joint angles back to output parameters (overwrite blending).
    pub fn apply(&mut self, params: &mut Params) {
        let mut strand_idx = 0usize;
        for s in &self.settings {
            let base = strand_idx;
            for sd in &s.strands {
                let joints = &self.strands[strand_idx];
                strand_idx += 1;
                let local = strand_idx - 1 - base;
                let base = sd.base_angle_deg.to_radians();
                for o in &s.outputs {
                    if o.strand != local {
                        continue;
                    }
                    let ji = (o.joint + 1).min(joints.len() - 1);
                    let parent = joints[ji - 1].pos;
                    let child = joints[ji].pos;
                    let a = (child - parent).y.atan2((child - parent).x);
                    let rel = ((a - base).to_degrees() / 45.0).clamp(-1.0, 1.0);
                    let d = &params.defs[o.pid];
                    let span = (d.max - d.def).abs().max((d.def - d.min).abs()).max(0.001);
                    // additive: several strands may drive the same parameter
                    params.add(o.pid, rel * o.scale * span);
                }
            }
        }
    }
}
