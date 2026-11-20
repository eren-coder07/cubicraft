// CubiCraft — character art & rig.
// An ORIGINAL chibi character ("Momi") drawn procedurally into a texture
// atlas at startup (the "PSD → parts" step), then rigged with a deformer
// tree (the "rigging" step). No Live2D assets are used anywhere.

use macroquad::prelude::*;
use std::collections::HashMap;

use crate::deform::*;
use crate::motion::{Clip, Curve, Key};
use crate::params::Params;
use crate::physics::{InputDef, OutputDef, PhysicsSetting, StrandDef};

// ---------- palette ----------
pub const SKIN: Color = Color::new(1.00, 0.894, 0.827, 1.00);
pub const SKIN_SH: Color = Color::new(0.945, 0.753, 0.659, 1.00);
pub const LINE: Color = Color::new(0.290, 0.231, 0.322, 1.00);
pub const HAIR: Color = Color::new(0.557, 0.847, 0.753, 1.00);
pub const HAIR_SH: Color = Color::new(0.392, 0.694, 0.596, 1.00);
pub const HAIR_HI: Color = Color::new(0.784, 0.949, 0.871, 1.00);
pub const HOODIE: Color = Color::new(0.992, 0.945, 0.875, 1.00);
pub const HOOD_SH: Color = Color::new(0.902, 0.824, 0.706, 1.00);
pub const HOOD_SH2: Color = Color::new(0.847, 0.760, 0.639, 1.00);
pub const CORAL: Color = Color::new(1.00, 0.561, 0.463, 1.00);
pub const CORAL_DK: Color = Color::new(0.878, 0.420, 0.333, 1.00);
pub const BLUSH: Color = Color::new(1.00, 0.627, 0.557, 1.00);
pub const MOUTH: Color = Color::new(0.620, 0.290, 0.341, 1.00);
pub const TONGUE: Color = Color::new(0.910, 0.475, 0.541, 1.00);
pub const BROW: Color = Color::new(0.369, 0.306, 0.431, 1.00);
pub const IRIS_DK: Color = Color::new(0.153, 0.396, 0.353, 1.00);
pub const IRIS: Color = Color::new(0.243, 0.608, 0.522, 1.00);
pub const IRIS_HI: Color = Color::new(0.435, 0.769, 0.686, 1.00);
pub const PUPIL: Color = Color::new(0.078, 0.180, 0.165, 1.00);
pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);

pub const ATLAS_PPU: f32 = 560.0; // atlas pixels per model unit
const PAD: f32 = 0.02; // part padding in model units

// ---------- shape building ----------

#[derive(Clone)]
pub struct PartShape {
    pub polys: Vec<(Vec<Vec2>, Color)>,
    pub strokes: Vec<(Vec<Vec2>, f32, Color)>,
    pub outline_scale: f32,
    pub outline_color: Color,
}

impl PartShape {
    fn new() -> Self {
        PartShape { polys: vec![], strokes: vec![], outline_scale: 0.0, outline_color: LINE }
    }
    fn poly(&mut self, pts: Vec<Vec2>, color: Color) {
        self.polys.push((pts, color));
    }
    fn stroke(&mut self, pts: Vec<Vec2>, w: f32, color: Color) {
        self.strokes.push((pts, w, color));
    }
}

fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32, n: usize) -> Vec<Vec2> {
    (0..n)
        .map(|i| {
            let a = std::f32::consts::TAU * i as f32 / n as f32;
            Vec2::new(cx + a.cos() * rx, cy + a.sin() * ry)
        })
        .collect()
}

fn chaikin(pts: &[Vec2], iters: u32) -> Vec<Vec2> {
    let mut cur: Vec<Vec2> = pts.to_vec();
    for _ in 0..iters {
        let mut next = Vec::with_capacity(cur.len() * 2);
        let n = cur.len();
        for i in 0..n {
            let a = cur[i];
            let b = cur[(i + 1) % n];
            next.push(a * 0.75 + b * 0.25);
            next.push(a * 0.25 + b * 0.75);
        }
        cur = next;
    }
    cur
}

fn mirror(pts: &[Vec2], cx: f32) -> Vec<Vec2> {
    let mut v: Vec<Vec2> = pts.iter().map(|p| Vec2::new(2.0 * cx - p.x, p.y)).collect();
    v.reverse();
    v
}

fn capsule(p0: Vec2, p1: Vec2, r: f32) -> Vec<Vec2> {
    let d = (p1 - p0).normalize();
    let base = d.y.atan2(d.x);
    let mut pts = Vec::new();
    let segs = 14;
    // arc around p1 from base-90 to base+90
    for i in 0..=segs {
        let a = base - std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32 / segs as f32);
        pts.push(p1 + vec2(a.cos(), a.sin()) * r);
    }
    // arc around p0 from base+90 to base+270
    for i in 0..=segs {
        let a = base + std::f32::consts::FRAC_PI_2
            + std::f32::consts::PI * (i as f32 / segs as f32);
        pts.push(p0 + vec2(a.cos(), a.sin()) * r);
    }
    pts
}

fn bez3(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, n: usize) -> Vec<Vec2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let a = (1.0 - t) * (1.0 - t) * (1.0 - t);
            let b = 3.0 * t * (1.0 - t) * (1.0 - t);
            let c = 3.0 * t * t * (1.0 - t);
            let d = t * t * t;
            Vec2::new(
                a * p0.x + b * p1.x + c * p2.x + d * p3.x,
                a * p0.y + b * p1.y + c * p2.y + d * p3.y,
            )
        })
        .collect()
}

// ---------- parts ----------

#[derive(Clone)]
pub struct Part {
    pub name: &'static str,
    pub shape: PartShape,
    pub div: (usize, usize),
    pub rect: Rect,      // filled after shape build (units)
    pub atlas: Rect,     // filled after packing (px)
}

fn finish_part(mut p: Part) -> Part {
    let mut b: Option<Rect> = None;
    let mut grow = |r: &mut Option<Rect>, pts: &[Vec2]| {
        for q in pts {
            *r = Some(match r {
                None => Rect::new(q.x, q.y, 0.0, 0.0),
                Some(b) => {
                    let x0 = b.x.min(q.x);
                    let y0 = b.y.min(q.y);
                    let x1 = (b.x + b.w).max(q.x);
                    let y1 = (b.y + b.h).max(q.y);
                    Rect::new(x0, y0, x1 - x0, y1 - y0)
                }
            });
        }
    };
    for (pts, _) in &p.shape.polys {
        grow(&mut b, pts);
    }
    for (pts, w, _) in &p.shape.strokes {
        let mut padded = pts.clone();
        // approximate stroke width in bounds
        for q in pts.iter() {
            padded.push(*q);
        }
        let _ = w;
        grow(&mut b, &padded);
    }
    let b = b.unwrap_or(Rect::new(0.0, 0.0, 0.01, 0.01));
    p.rect = Rect::new(b.x - PAD, b.y - PAD, b.w + 2.0 * PAD, b.h + 2.0 * PAD);
    p
}

fn add(parts: &mut Vec<Part>, name: &'static str, shape: PartShape, div: (usize, usize)) {
    parts.push(finish_part(Part { name, shape, div, rect: Rect::default(), atlas: Rect::default() }));
}

pub fn build_parts() -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();

    // ---- ground shadow ----
    {
        let mut s = PartShape::new();
        s.poly(ellipse(1.5, 2.665, 0.52, 0.075, 40), Color::new(0.043, 0.031, 0.071, 0.85));
        add(&mut parts, "Shadow", s, (2, 2));
    }

    // ---- back hair ----
    {
        let mut s = PartShape::new();
        let pts = chaikin(
            &[
                vec2(0.92, 1.02),
                vec2(1.00, 0.62),
                vec2(1.28, 0.44),
                vec2(1.5, 0.415),
                vec2(1.72, 0.44),
                vec2(2.00, 0.62),
                vec2(2.08, 1.02),
                vec2(2.10, 1.45),
                vec2(2.02, 1.74),
                vec2(1.84, 1.87),
                vec2(1.66, 1.80),
                vec2(1.5, 1.86),
                vec2(1.34, 1.80),
                vec2(1.16, 1.87),
                vec2(0.98, 1.74),
                vec2(0.90, 1.45),
            ],
            2,
        );
        s.outline_scale = 1.02;
        s.poly(pts.clone(), HAIR);
        s.poly(chaikin(&[vec2(0.95, 1.45), vec2(1.02, 1.75), vec2(1.2, 1.87), vec2(1.36, 1.82), vec2(1.34, 1.55), vec2(1.18, 1.52), vec2(0.96, 1.30)], 2), HAIR_SH);
        s.poly(chaikin(&[vec2(1.28, 0.50), vec2(1.5, 0.46), vec2(1.72, 0.52), vec2(1.9, 0.70), vec2(1.6, 0.62), vec2(1.4, 0.58)], 2), HAIR_HI);
        add(&mut parts, "BackHair", s, (4, 4));
    }

    // ---- neck ----
    {
        let mut s = PartShape::new();
        s.outline_scale = 1.0;
        s.poly(
            vec![vec2(1.40, 1.44), vec2(1.60, 1.44), vec2(1.61, 1.66), vec2(1.39, 1.66)],
            SKIN,
        );
        s.poly(vec![vec2(1.40, 1.44), vec2(1.60, 1.44), vec2(1.585, 1.535), vec2(1.415, 1.535)], SKIN_SH);
        add(&mut parts, "Neck", s, (2, 2));
    }

    // ---- torso (hoodie) ----
    {
        let mut s = PartShape::new();
        s.outline_scale = 1.018;
        s.poly(
            chaikin(
                &[
                    vec2(1.19, 1.63),
                    vec2(1.5, 1.585),
                    vec2(1.81, 1.63),
                    vec2(1.94, 1.93),
                    vec2(1.97, 2.20),
                    vec2(1.87, 2.44),
                    vec2(1.5, 2.50),
                    vec2(1.13, 2.44),
                    vec2(1.03, 2.20),
                    vec2(1.06, 1.93),
                ],
                2,
            ),
            HOODIE,
        );
        // right-side shade
        s.poly(
            chaikin(
                &[
                    vec2(1.74, 1.62),
                    vec2(1.92, 1.90),
                    vec2(1.95, 2.18),
                    vec2(1.85, 2.42),
                    vec2(1.62, 2.47),
                    vec2(1.76, 2.18),
                    vec2(1.76, 1.90),
                ],
                2,
            ),
            Color::new(HOOD_SH.r, HOOD_SH.g, HOOD_SH.b, 0.38),
        );
        // collar
        s.poly(ellipse(1.5, 1.625, 0.115, 0.052, 24), HOOD_SH);
        s.poly(ellipse(1.5, 1.625, 0.075, 0.033, 24), SKIN);
        // bottom band
        s.poly(
            vec![vec2(1.10, 2.335), vec2(1.90, 2.335), vec2(1.87, 2.45), vec2(1.13, 2.45)],
            HOOD_SH,
        );
        // legs
        s.poly(vec![vec2(1.26, 2.42), vec2(1.44, 2.42), vec2(1.44, 2.60), vec2(1.26, 2.60)], LINE);
        s.poly(vec![vec2(1.56, 2.42), vec2(1.74, 2.42), vec2(1.74, 2.60), vec2(1.56, 2.60)], LINE);
        s.poly(ellipse(1.365, 2.60, 0.105, 0.045, 20), CORAL);
        s.poly(ellipse(1.645, 2.60, 0.105, 0.045, 20), CORAL);
        // drawstrings
        s.stroke(vec![vec2(1.44, 1.65), vec2(1.425, 1.74), vec2(1.43, 1.80)], 0.020, CORAL_DK);
        s.stroke(vec![vec2(1.56, 1.65), vec2(1.575, 1.74), vec2(1.57, 1.80)], 0.020, CORAL_DK);
        // pocket hints
        s.stroke(vec![vec2(1.28, 2.12), vec2(1.42, 2.22)], 0.018, HOOD_SH2);
        s.stroke(vec![vec2(1.72, 2.12), vec2(1.58, 2.22)], 0.018, HOOD_SH2);
        add(&mut parts, "Torso", s, (4, 4));
    }

    // ---- arms ----
    {
        let mut s = PartShape::new();
        s.outline_scale = 1.05;
        s.poly(capsule(vec2(1.15, 1.72), vec2(1.02, 2.10), 0.093), HOODIE);
        s.poly(ellipse(1.01, 2.14, 0.075, 0.072, 20), SKIN);
        add(&mut parts, "ArmL", s, (3, 3));
        let mut s = PartShape::new();
        s.outline_scale = 1.05;
        s.poly(capsule(vec2(1.85, 1.72), vec2(1.98, 2.10), 0.093), HOODIE);
        s.poly(ellipse(1.99, 2.14, 0.075, 0.072, 20), SKIN);
        add(&mut parts, "ArmR", s, (3, 3));
    }

    // ---- face ----
    {
        let mut s = PartShape::new();
        s.outline_scale = 1.022;
        s.poly(
            chaikin(
                &[
                    vec2(1.5, 0.505),
                    vec2(1.72, 0.545),
                    vec2(1.90, 0.68),
                    vec2(1.995, 0.90),
                    vec2(2.02, 1.13),
                    vec2(1.99, 1.33),
                    vec2(1.88, 1.47),
                    vec2(1.72, 1.555),
                    vec2(1.5, 1.585),
                    vec2(1.28, 1.555),
                    vec2(1.12, 1.47),
                    vec2(1.01, 1.33),
                    vec2(0.98, 1.13),
                    vec2(1.005, 0.90),
                    vec2(1.10, 0.68),
                    vec2(1.28, 0.545),
                ],
                2,
            ),
            SKIN,
        );
        // hairline shadow under bangs
        s.poly(ellipse(1.5, 0.965, 0.34, 0.105, 32), Color::new(SKIN_SH.r, SKIN_SH.g, SKIN_SH.b, 0.30));
        // nose
        s.poly(ellipse(1.5, 1.255, 0.017, 0.013, 12), Color::new(SKIN_SH.r, SKIN_SH.g, SKIN_SH.b, 0.9));
        add(&mut parts, "Face", s, (6, 5));
    }

    // ---- blush ----
    {
        let mut s = PartShape::new();
        s.poly(ellipse(1.135, 1.262, 0.075, 0.043, 24), BLUSH);
        s.poly(ellipse(1.13, 1.262, 0.04, 0.02, 16), Color::new(1.0, 0.75, 0.68, 0.8));
        add(&mut parts, "BlushL", s, (2, 2));
        let mut s = PartShape::new();
        s.poly(mirror(&ellipse(1.135, 1.262, 0.075, 0.043, 24), 1.5), BLUSH);
        s.poly(mirror(&ellipse(1.13, 1.262, 0.04, 0.02, 16), 1.5), Color::new(1.0, 0.75, 0.68, 0.8));
        add(&mut parts, "BlushR", s, (2, 2));
    }

    // ---- brows ----
    {
        let mut s = PartShape::new();
        s.stroke(
            vec![vec2(1.17, 0.885), vec2(1.25, 0.855), vec2(1.33, 0.885)],
            0.030,
            BROW,
        );
        add(&mut parts, "BrowL", s, (2, 2));
        let mut s = PartShape::new();
        s.stroke(
            vec![vec2(1.67, 0.885), vec2(1.75, 0.855), vec2(1.83, 0.885)],
            0.030,
            BROW,
        );
        add(&mut parts, "BrowR", s, (2, 2));
    }

    // ---- eyes ----
    {
        let mut s = PartShape::new();
        s.poly(ellipse(1.245, 1.105, 0.118, 0.132, 36), WHITE);
        parts.push(finish_part(Part { name: "EyeWhiteL", shape: s, div: (2, 2), rect: Rect::default(), atlas: Rect::default() }));
        let mut s = PartShape::new();
        s.poly(ellipse(1.755, 1.105, 0.118, 0.132, 36), WHITE);
        parts.push(finish_part(Part { name: "EyeWhiteR", shape: s, div: (2, 2), rect: Rect::default(), atlas: Rect::default() }));
    }
    {
        let mut s = PartShape::new();
        s.poly(ellipse(1.245, 1.125, 0.085, 0.090, 28), IRIS_DK);
        s.poly(ellipse(1.245, 1.125, 0.064, 0.068, 24), IRIS);
        s.poly(ellipse(1.245, 1.14, 0.040, 0.042, 20), IRIS_HI);
        s.poly(ellipse(1.245, 1.14, 0.020, 0.021, 14), PUPIL);
        s.poly(ellipse(1.222, 1.098, 0.024, 0.026, 14), WHITE);
        s.poly(ellipse(1.268, 1.158, 0.012, 0.013, 10), Color::new(1.0, 1.0, 1.0, 0.85));
        add(&mut parts, "EyeIrisL", s, (2, 2));
        let mut s = PartShape::new();
        s.poly(ellipse(1.755, 1.125, 0.085, 0.090, 28), IRIS_DK);
        s.poly(ellipse(1.755, 1.125, 0.064, 0.068, 24), IRIS);
        s.poly(ellipse(1.755, 1.14, 0.040, 0.042, 20), IRIS_HI);
        s.poly(ellipse(1.755, 1.14, 0.020, 0.021, 14), PUPIL);
        s.poly(ellipse(1.732, 1.098, 0.024, 0.026, 14), WHITE);
        s.poly(ellipse(1.778, 1.158, 0.012, 0.013, 10), Color::new(1.0, 1.0, 1.0, 0.85));
        add(&mut parts, "EyeIrisR", s, (2, 2));
    }
    {
        let mut s = PartShape::new();
        let cx = 1.245;
        let cy = 1.105;
        let arc: Vec<Vec2> = (0..=14)
            .map(|i| {
                let a = std::f32::consts::PI + 0.25 + (std::f32::consts::PI - 0.5) * i as f32 / 14.0;
                Vec2::new(cx + a.cos() * 0.128, cy + a.sin() * 0.150)
            })
            .collect();
        s.stroke(arc, 0.038, LINE);
        add(&mut parts, "EyeLashL", s, (3, 3));
        let mut s = PartShape::new();
        let cx = 1.755;
        let arc: Vec<Vec2> = (0..=14)
            .map(|i| {
                let a = std::f32::consts::PI + 0.25 + (std::f32::consts::PI - 0.5) * i as f32 / 14.0;
                Vec2::new(cx + a.cos() * 0.128, cy + a.sin() * 0.150)
            })
            .collect();
        s.stroke(arc, 0.038, LINE);
        add(&mut parts, "EyeLashR", s, (3, 3));
    }
    {
        // happy (smile) eye: "^" arc
        let mut s = PartShape::new();
        let cx = 1.245;
        s.stroke(bez3(vec2(cx - 0.11, 1.14), vec2(cx - 0.03, 1.02), vec2(cx + 0.03, 1.02), vec2(cx + 0.11, 1.14), 12), 0.036, LINE);
        add(&mut parts, "EyeSmileL", s, (2, 2));
        let mut s = PartShape::new();
        let cx = 1.755;
        s.stroke(bez3(vec2(cx - 0.11, 1.14), vec2(cx - 0.03, 1.02), vec2(cx + 0.03, 1.02), vec2(cx + 0.11, 1.14), 12), 0.036, LINE);
        add(&mut parts, "EyeSmileR", s, (2, 2));
    }

    // ---- mouth ----
    {
        let mut s = PartShape::new();
        s.stroke(
            vec![vec2(1.405, 1.378), vec2(1.45, 1.386), vec2(1.5, 1.388), vec2(1.55, 1.386), vec2(1.595, 1.378)],
            0.022,
            MOUTH,
        );
        add(&mut parts, "MouthClosed", s, (3, 2));
        let mut s = PartShape::new();
        s.poly(ellipse(1.5, 1.40, 0.085, 0.062, 28), MOUTH);
        s.poly(ellipse(1.5, 1.425, 0.055, 0.030, 20), TONGUE);
        s.poly(
            vec![vec2(1.44, 1.345), vec2(1.56, 1.345), vec2(1.56, 1.368), vec2(1.44, 1.368)],
            WHITE,
        );
        add(&mut parts, "MouthOpen", s, (3, 3));
    }

    // ---- bangs ----
    {
        let mut s = PartShape::new();
        let pts = chaikin(
            &[
                vec2(1.00, 0.84),
                vec2(1.03, 0.60),
                vec2(1.22, 0.475),
                vec2(1.5, 0.44),
                vec2(1.78, 0.475),
                vec2(1.97, 0.60),
                vec2(2.00, 0.84),
                vec2(1.955, 0.92),
                vec2(1.87, 1.075),
                vec2(1.77, 0.905),
                vec2(1.69, 1.03),
                vec2(1.60, 0.90),
                vec2(1.50, 1.00),
                vec2(1.40, 0.90),
                vec2(1.31, 1.03),
                vec2(1.23, 0.905),
                vec2(1.13, 1.075),
                vec2(1.045, 0.92),
            ],
            1,
        );
        s.outline_scale = 1.03;
        s.poly(pts.clone(), HAIR);
        // shine band
        let arc: Vec<Vec2> = (0..=12)
            .map(|i| {
                let a = std::f32::consts::PI + 0.4 + (std::f32::consts::PI - 0.8) * i as f32 / 12.0;
                Vec2::new(1.5 + a.cos() * 0.36, 0.66 + a.sin() * 0.17)
            })
            .collect();
        s.stroke(arc, 0.045, HAIR_HI);
        add(&mut parts, "Bangs", s, (6, 4));
    }

    // ---- ahoge ----
    {
        let mut s = PartShape::new();
        let pts = bez3(vec2(1.43, 0.58), vec2(1.45, 0.42), vec2(1.58, 0.34), vec2(1.70, 0.40), 12);
        s.stroke(pts.clone(), 0.062, LINE);
        s.stroke(bez3(vec2(1.43, 0.58), vec2(1.45, 0.42), vec2(1.58, 0.34), vec2(1.70, 0.40), 12), 0.040, HAIR);
        s.stroke(bez3(vec2(1.43, 0.58), vec2(1.44, 0.50), vec2(1.50, 0.42), vec2(1.58, 0.385), 10), 0.022, HAIR_HI);
        add(&mut parts, "Ahoge", s, (3, 3));
    }

    // ---- side hair ----
    {
        let mut s = PartShape::new();
        let pts = chaikin(
            &[
                vec2(0.97, 0.70),
                vec2(1.10, 0.68),
                vec2(1.14, 0.95),
                vec2(1.145, 1.30),
                vec2(1.115, 1.58),
                vec2(1.04, 1.80),
                vec2(0.95, 1.62),
                vec2(0.885, 1.35),
                vec2(0.875, 1.00),
            ],
            2,
        );
        s.outline_scale = 1.04;
        s.poly(pts.clone(), HAIR);
        s.poly(
            chaikin(&[vec2(1.10, 1.30), vec2(1.12, 1.58), vec2(1.05, 1.76), vec2(0.97, 1.60), vec2(0.94, 1.30)], 2),
            HAIR_SH,
        );
        add(&mut parts, "SideHairL", s, (3, 4));
        let mut s = PartShape::new();
        let pts = mirror(
            &chaikin(
                &[
                    vec2(0.97, 0.70),
                    vec2(1.10, 0.68),
                    vec2(1.14, 0.95),
                    vec2(1.145, 1.30),
                    vec2(1.115, 1.58),
                    vec2(1.04, 1.80),
                    vec2(0.95, 1.62),
                    vec2(0.885, 1.35),
                    vec2(0.875, 1.00),
                ],
                2,
            ),
            1.5,
        );
        s.outline_scale = 1.04;
        s.poly(pts.clone(), HAIR);
        s.poly(
            mirror(
                &chaikin(&[vec2(1.10, 1.30), vec2(1.12, 1.58), vec2(1.05, 1.76), vec2(0.97, 1.60), vec2(0.94, 1.30)], 2),
                1.5,
            ),
            HAIR_SH,
        );
        add(&mut parts, "SideHairR", s, (3, 4));
    }

    parts
}

// ---------- atlas packing ----------

pub fn pack_atlas(parts: &mut [Part]) {
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by(|a, b| {
        let ha = parts[*a].rect.h * ATLAS_PPU;
        let hb = parts[*b].rect.h * ATLAS_PPU;
        hb.partial_cmp(&ha).unwrap()
    });
    let mut x = 0f32;
    let mut y = 0f32;
    let mut row_h = 0f32;
    let pad = 8.0;
    for i in order {
        let w = parts[i].rect.w * ATLAS_PPU + pad;
        let h = parts[i].rect.h * ATLAS_PPU + pad;
        if x + w > ATLAS_W as f32 {
            x = 0.0;
            y += row_h;
            row_h = 0.0;
        }
        parts[i].atlas = Rect::new(x, y, w, h);
        x += w;
        row_h = row_h.max(h);
        if y + row_h > ATLAS_H as f32 {
            panic!("atlas overflow — increase ATLAS size");
        }
    }
}

// ---------- baking ----------

pub fn bake_atlas(parts: &[Part]) {
    let ppu = ATLAS_PPU;
    for p in parts {
        let a = p.atlas;
        let r = p.rect;
        let to_px = |v: Vec2| -> Vec2 {
            Vec2::new(a.x + (v.x - r.x) * ppu, a.y + (v.y - r.y) * ppu)
        };
        // outline pass
        if p.shape.outline_scale > 0.0 {
            for (pts, _) in &p.shape.polys {
                let c = pts.iter().fold(Vec2::ZERO, |acc, q| acc + *q) / pts.len() as f32;
                let scaled: Vec<Vec2> = pts.iter().map(|q| c + (*q - c) * p.shape.outline_scale).collect();
                let px: Vec<Vec2> = scaled.iter().map(|q| to_px(*q)).collect();
                for t in ear_clip(&px) {
                    draw_triangle(px[t.0], px[t.1], px[t.2], p.shape.outline_color);
                }
            }
        }
        // fill pass
        for (pts, color) in &p.shape.polys {
            let px: Vec<Vec2> = pts.iter().map(|q| to_px(*q)).collect();
            for t in ear_clip(&px) {
                draw_triangle(px[t.0], px[t.1], px[t.2], *color);
            }
        }
        // stroke pass
        for (pts, w, color) in &p.shape.strokes {
            let wpx = w * ppu;
            let px: Vec<Vec2> = pts.iter().map(|q| to_px(*q)).collect();
            for wnd in px.windows(2) {
                draw_line(wnd[0].x, wnd[0].y, wnd[1].x, wnd[1].y, wpx, *color);
            }
            for q in &px {
                draw_circle(q.x, q.y, wpx * 0.5, *color);
            }
        }
    }
}

/// Ear-clipping triangulation for simple (possibly concave) polygons.
pub fn ear_clip(pts: &[Vec2]) -> Vec<(usize, usize, usize)> {
    let n = pts.len();
    let mut tris = Vec::new();
    if n < 3 {
        return tris;
    }
    // ensure CCW in screen coords (y down => visual CW); compute signed area
    let mut area = 0.0;
    for i in 0..n {
        let a = pts[i];
        let b = pts[(i + 1) % n];
        area += a.x * b.y - b.x * a.y;
    }
    let mut idx: Vec<usize> = (0..n).collect();
    if area < 0.0 {
        idx.reverse();
    }
    let cross = |a: Vec2, b: Vec2, c: Vec2| -> f32 { (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x) };
    let point_in_tri = |p: Vec2, a: Vec2, b: Vec2, c: Vec2| -> bool {
        let d1 = cross(a, b, p);
        let d2 = cross(b, c, p);
        let d3 = cross(c, a, p);
        (d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0) || (d1 <= 0.0 && d2 <= 0.0 && d3 <= 0.0)
    };
    let mut guard = 0;
    while idx.len() > 3 && guard < 5000 {
        guard += 1;
        let mut clipped = false;
        let m = idx.len();
        for i in 0..m {
            let i0 = idx[(i + m - 1) % m];
            let i1 = idx[i];
            let i2 = idx[(i + 1) % m];
            let a = pts[i0];
            let b = pts[i1];
            let c = pts[i2];
            if cross(a, b, c) <= 1e-9 {
                continue; // reflex
            }
            let mut contains = false;
            for &j in &idx {
                if j == i0 || j == i1 || j == i2 {
                    continue;
                }
                if point_in_tri(pts[j], a, b, c) {
                    contains = true;
                    break;
                }
            }
            if contains {
                continue;
            }
            tris.push((i0, i1, i2));
            idx.remove(i);
            clipped = true;
            break;
        }
        if !clipped {
            break;
        }
    }
    if idx.len() == 3 {
        tris.push((idx[0], idx[1], idx[2]));
    }
    tris
}

// ---------- rig construction ----------

pub struct PartMeta {
    pub rect: Rect,
    pub atlas: Rect,
    pub div: (usize, usize),
}

pub struct Rig {
    pub root: Node,
    pub parts: HashMap<&'static str, PartMeta>,
}

fn warp(rect: Rect, dx: usize, dy: usize, drivers: Vec<Driver>, children: Vec<Node>) -> Node {
    Node::Warp {
        def: WarpDef { rect, div_x: dx, div_y: dy, drivers, base_offsets: vec![], keyforms: vec![] },
        children,
    }
}

fn rot(pivot: Vec2, drivers: Vec<Driver>, children: Vec<Node>) -> Node {
    Node::Rot { def: RotDef { pivot, drivers }, children }
}

#[allow(clippy::too_many_arguments)]
fn mesh(parts: &HashMap<&'static str, PartMeta>, name: &str, drivers: Vec<Driver>, base_op: f32, opac: Vec<(usize, f32)>) -> Node {
    let pm = &parts[name];
    Node::Mesh(MeshDef {
        rect: pm.rect,
        div_x: pm.div.0,
        div_y: pm.div.1,
        uv: pm.atlas,
        part: name.to_string(),
        drivers,
        base_opacity: base_op,
        opac_drivers: opac,
    })
}

pub fn build_rig(params: &Params, parts: &[Part]) -> Rig {
    let map: HashMap<&'static str, PartMeta> = parts
        .iter()
        .map(|p| {
            (
                p.name,
                PartMeta { rect: p.rect, atlas: p.atlas, div: p.div },
            )
        })
        .collect();
    let P = |id: &str| params.pid(id);

    let base_x = P("ParamBaseX");
    let base_y = P("ParamBaseY");
    let breath = P("ParamBreath");
    let ang_x = P("ParamAngleX");
    let ang_y = P("ParamAngleY");
    let ang_z = P("ParamAngleZ");
    let body_x = P("ParamBodyAngleX");
    let body_y = P("ParamBodyAngleY");
    let body_z = P("ParamBodyAngleZ");
    let eye_l_open = P("ParamEyeLOpen");
    let eye_r_open = P("ParamEyeROpen");
    let eye_l_smile = P("ParamEyeLSmile");
    let eye_r_smile = P("ParamEyeRSmile");
    let ball_x = P("ParamEyeBallX");
    let ball_y = P("ParamEyeBallY");
    let brow_ly = P("ParamBrowLY");
    let brow_ry = P("ParamBrowRY");
    let brow_la = P("ParamBrowLAngle");
    let brow_ra = P("ParamBrowRAngle");
    let mouth_form = P("ParamMouthForm");
    let mouth_open = P("ParamMouthOpenY");
    let cheek = P("ParamCheek");
    let hair_front = P("ParamHairFront");
    let hair_side = P("ParamHairSide");
    let hair_back = P("ParamHairBack");
    let hair_fluffy = P("ParamHairFluffy");
    let _shoulder = P("ParamShoulderY");

    let root = Node::Warp {
        def: WarpDef {
            rect: Rect::new(0.0, 0.0, 3.0, 3.0),
            div_x: 2,
            div_y: 2,
            drivers: vec![
                Driver { pid: base_x, kind: DriverKind::ShiftX { scale: 0.05, w: Weight::One } },
                Driver { pid: base_y, kind: DriverKind::ShiftY { scale: -0.08, w: Weight::One } },
            ],
            base_offsets: vec![],
            keyforms: vec![],
        },
        children: vec![
            mesh(&map, "Shadow", vec![], 1.0, vec![]),
            // back hair: behind the body, loosely following the head
            warp(
                Rect::new(0.76, 0.34, 1.48, 1.68),
                4,
                4,
                vec![
                    Driver { pid: ang_x, kind: DriverKind::ShiftX { scale: 0.050, w: Weight::One } },
                    Driver { pid: ang_z, kind: DriverKind::RotBy { pivot: vec2(1.5, 1.0), deg: 8.0, w: Weight::One } },
                    Driver { pid: hair_back, kind: DriverKind::ShiftX { scale: 0.045, w: Weight::VGrad(0.5, 1.0) } },
                    Driver { pid: breath, kind: DriverKind::ShiftY { scale: -0.008, w: Weight::One } },
                ],
                vec![mesh(&map, "BackHair", vec![], 1.0, vec![])],
            ),
            // body
            rot(
                vec2(1.5, 1.55),
                vec![
                    Driver { pid: body_z, kind: DriverKind::RotBy { pivot: vec2(1.5, 1.55), deg: 7.0, w: Weight::One } },
                    Driver { pid: body_x, kind: DriverKind::RotBy { pivot: vec2(1.5, 2.10), deg: 4.0, w: Weight::One } },
                ],
                vec![
                    warp(
                        Rect::new(0.85, 1.40, 1.30, 1.32),
                        2,
                        2,
                        vec![
                            Driver { pid: breath, kind: DriverKind::ScaleY { cv: 1.0, k: -0.030, w: Weight::One } },
                            Driver { pid: breath, kind: DriverKind::ScaleX { cu: 0.5, k: 0.012, w: Weight::One } },
                            Driver { pid: body_y, kind: DriverKind::ShiftY { scale: 0.02, w: Weight::One } },
                        ],
                        vec![
                            mesh(&map, "Neck", vec![], 1.0, vec![]),
                            mesh(&map, "Torso", vec![], 1.0, vec![]),
                            rot(
                                vec2(1.86, 1.68),
                                vec![Driver { pid: P("ParamArmRA"), kind: DriverKind::RotBy { pivot: vec2(1.86, 1.68), deg: 20.0, w: Weight::One } }],
                                vec![mesh(&map, "ArmR", vec![], 1.0, vec![])],
                            ),
                            rot(
                                vec2(1.14, 1.68),
                                vec![Driver { pid: P("ParamArmLA"), kind: DriverKind::RotBy { pivot: vec2(1.14, 1.68), deg: 20.0, w: Weight::One } }],
                                vec![mesh(&map, "ArmL", vec![], 1.0, vec![])],
                            ),
                        ],
                    ),
                ],
            ),
            // head
            rot(
                vec2(1.5, 1.52),
                vec![Driver { pid: ang_z, kind: DriverKind::RotBy { pivot: vec2(1.5, 1.52), deg: 10.0, w: Weight::One } }],
                vec![
                    warp(
                        Rect::new(0.80, 0.38, 1.40, 1.34),
                        5,
                        4,
                        vec![
                            Driver { pid: ang_x, kind: DriverKind::ShiftX { scale: 0.058, w: Weight::EdgeX(0.6) } },
                            Driver { pid: ang_y, kind: DriverKind::ShiftY { scale: 0.055, w: Weight::VGrad(0.7, 0.6) } },
                            Driver { pid: breath, kind: DriverKind::ShiftY { scale: -0.010, w: Weight::One } },
                        ],
                        vec![
                            mesh(&map, "Face", vec![], 1.0, vec![]),
                            mesh(&map, "BlushL", vec![], 0.45, vec![(cheek, 1.0)]),
                            mesh(&map, "BlushR", vec![], 0.45, vec![(cheek, 1.0)]),
                            rot(
                                vec2(1.25, 0.92),
                                vec![Driver { pid: brow_la, kind: DriverKind::RotBy { pivot: vec2(1.25, 0.92), deg: -7.0, w: Weight::One } }],
                                vec![mesh(&map, "BrowL", vec![Driver { pid: brow_ly, kind: DriverKind::ShiftY { scale: -0.028, w: Weight::One } }], 1.0, vec![])],
                            ),
                            rot(
                                vec2(1.75, 0.92),
                                vec![Driver { pid: brow_ra, kind: DriverKind::RotBy { pivot: vec2(1.75, 0.92), deg: 7.0, w: Weight::One } }],
                                vec![mesh(&map, "BrowR", vec![Driver { pid: brow_ry, kind: DriverKind::ShiftY { scale: -0.028, w: Weight::One } }], 1.0, vec![])],
                            ),
                            // left eye group
                            warp(
                                Rect::new(1.07, 0.90, 0.36, 0.42),
                                2,
                                2,
                                vec![Driver { pid: eye_l_open, kind: DriverKind::ScaleY { cv: 0.0, k: -0.92, w: Weight::One } }],
                                vec![
                                    mesh(&map, "EyeWhiteL", vec![], 1.0, vec![(eye_l_smile, -1.0)]),
                                    mesh(
                                        &map,
                                        "EyeIrisL",
                                        vec![
                                            Driver { pid: ball_x, kind: DriverKind::ShiftX { scale: 0.032, w: Weight::One } },
                                            Driver { pid: ball_y, kind: DriverKind::ShiftY { scale: 0.034, w: Weight::One } },
                                        ],
                                        1.0,
                                        vec![(eye_l_smile, -1.0)],
                                    ),
                                    mesh(&map, "EyeLashL", vec![], 1.0, vec![(eye_l_smile, -1.0)]),
                                ],
                            ),
                            mesh(&map, "EyeSmileL", vec![], 0.0, vec![(eye_l_smile, 1.0)]),
                            // right eye group
                            warp(
                                Rect::new(1.57, 0.90, 0.36, 0.42),
                                2,
                                2,
                                vec![Driver { pid: eye_r_open, kind: DriverKind::ScaleY { cv: 0.0, k: -0.92, w: Weight::One } }],
                                vec![
                                    mesh(&map, "EyeWhiteR", vec![], 1.0, vec![(eye_r_smile, -1.0)]),
                                    mesh(
                                        &map,
                                        "EyeIrisR",
                                        vec![
                                            Driver { pid: ball_x, kind: DriverKind::ShiftX { scale: 0.032, w: Weight::One } },
                                            Driver { pid: ball_y, kind: DriverKind::ShiftY { scale: 0.034, w: Weight::One } },
                                        ],
                                        1.0,
                                        vec![(eye_r_smile, -1.0)],
                                    ),
                                    mesh(&map, "EyeLashR", vec![], 1.0, vec![(eye_r_smile, -1.0)]),
                                ],
                            ),
                            mesh(&map, "EyeSmileR", vec![], 0.0, vec![(eye_r_smile, 1.0)]),
                            // mouth
                            warp(
                                Rect::new(1.30, 1.28, 0.40, 0.26),
                                3,
                                2,
                                vec![
                                    Driver { pid: mouth_form, kind: DriverKind::ShiftY { scale: -0.014, w: Weight::One } },
                                    Driver { pid: mouth_form, kind: DriverKind::ScaleX { cu: 0.5, k: 0.05, w: Weight::One } },
                                ],
                                vec![
                                    mesh(&map, "MouthClosed", vec![], 1.0, vec![(mouth_open, -0.92)]),
                                    mesh(
                                        &map,
                                        "MouthOpen",
                                        vec![Driver { pid: mouth_open, kind: DriverKind::ScaleX { cu: 0.5, k: 0.12, w: Weight::One } }],
                                        0.0,
                                        vec![(mouth_open, 1.0)],
                                    ),
                                ],
                            ),
                            // bangs
                            warp(
                                Rect::new(0.92, 0.42, 1.16, 0.72),
                                6,
                                4,
                                vec![
                                    Driver { pid: hair_front, kind: DriverKind::ShiftX { scale: 0.050, w: Weight::VGrad(0.35, 1.0) } },
                                    Driver { pid: ang_x, kind: DriverKind::ShiftX { scale: 0.022, w: Weight::One } },
                                ],
                                vec![mesh(&map, "Bangs", vec![], 1.0, vec![])],
                            ),
                            // ahoge
                            warp(
                                Rect::new(1.28, 0.28, 0.58, 0.42),
                                3,
                                3,
                                vec![Driver { pid: hair_front, kind: DriverKind::ShiftX { scale: 0.065, w: Weight::VGrad(0.2, 1.0) } }],
                                vec![mesh(&map, "Ahoge", vec![], 1.0, vec![])],
                            ),
                            // side hair
                            warp(
                                Rect::new(0.76, 0.58, 0.48, 1.32),
                                3,
                                4,
                                vec![
                                    Driver { pid: hair_side, kind: DriverKind::ShiftX { scale: 0.055, w: Weight::VGrad(0.3, 1.0) } },
                                    Driver { pid: hair_fluffy, kind: DriverKind::ShiftX { scale: -0.030, w: Weight::VGrad(0.2, 1.0) } },
                                    Driver { pid: ang_x, kind: DriverKind::ShiftX { scale: 0.018, w: Weight::One } },
                                ],
                                vec![mesh(&map, "SideHairL", vec![], 1.0, vec![])],
                            ),
                            warp(
                                Rect::new(1.76, 0.58, 0.48, 1.32),
                                3,
                                4,
                                vec![
                                    Driver { pid: hair_side, kind: DriverKind::ShiftX { scale: 0.055, w: Weight::VGrad(0.3, 1.0) } },
                                    Driver { pid: hair_fluffy, kind: DriverKind::ShiftX { scale: 0.030, w: Weight::VGrad(0.2, 1.0) } },
                                    Driver { pid: ang_x, kind: DriverKind::ShiftX { scale: 0.018, w: Weight::One } },
                                ],
                                vec![mesh(&map, "SideHairR", vec![], 1.0, vec![])],
                            ),
                        ],
                    ),
                ],
            ),
        ],
    };

    Rig { root, parts: map }
}

// ---------- physics settings ----------

pub fn build_physics(params: &Params) -> PhysicsSetting {
    PhysicsSetting {
        name: "HairSway".into(),
        inputs: vec![
            InputDef { pid: params.pid("ParamAngleX"), deg: 30.0 },
            InputDef { pid: params.pid("ParamAngleZ"), deg: 30.0 },
            InputDef { pid: params.pid("ParamBodyAngleX"), deg: 10.0 },
        ],
        strands: vec![
            StrandDef {
                anchor: vec2(1.5, 0.72),
                base_angle_deg: 90.0,
                n_joints: 1,
                rest_len: 0.14,
                stiffness: 3.5,
                inertial_mass: 1.0,
                damping: 4.5,
            },
            StrandDef {
                anchor: vec2(1.00, 0.80),
                base_angle_deg: 96.0,
                n_joints: 2,
                rest_len: 0.16,
                stiffness: 2.6,
                inertial_mass: 1.2,
                damping: 4.0,
            },
            StrandDef {
                anchor: vec2(2.00, 0.80),
                base_angle_deg: 84.0,
                n_joints: 2,
                rest_len: 0.16,
                stiffness: 2.6,
                inertial_mass: 1.2,
                damping: 4.0,
            },
            StrandDef {
                anchor: vec2(1.5, 0.62),
                base_angle_deg: 90.0,
                n_joints: 2,
                rest_len: 0.18,
                stiffness: 2.0,
                inertial_mass: 1.4,
                damping: 3.6,
            },
            StrandDef {
                anchor: vec2(1.45, 0.55),
                base_angle_deg: -70.0,
                n_joints: 1,
                rest_len: 0.12,
                stiffness: 1.8,
                inertial_mass: 0.6,
                damping: 3.0,
            },
        ],
        outputs: vec![
            OutputDef { strand: 0, joint: 0, pid: params.pid("ParamHairFront"), scale: 0.85 },
            OutputDef { strand: 1, joint: 1, pid: params.pid("ParamHairSide"), scale: 0.80 },
            OutputDef { strand: 1, joint: 1, pid: params.pid("ParamHairFluffy"), scale: 0.45 },
            OutputDef { strand: 2, joint: 1, pid: params.pid("ParamHairSide"), scale: 0.80 },
            OutputDef { strand: 3, joint: 1, pid: params.pid("ParamHairBack"), scale: 0.90 },
            OutputDef { strand: 4, joint: 0, pid: params.pid("ParamHairFront"), scale: 0.55 },
        ],
    }
}

// ---------- motions ----------

fn curve(name: &str, params: &Params, keys: &[(f32, f32)], add: bool) -> Curve {
    Curve {
        pid: params.pid(name),
        keys: keys.iter().map(|(t, v)| Key { t: *t, v: *v }).collect(),
        add,
    }
}

pub fn idle_motion(params: &Params) -> Clip {
    Clip {
        name: "IdleSway".into(),
        dur: 12.0,
        looped: true,
        curves: vec![
            curve("ParamAngleZ", params, &[(0.0, 0.0), (3.0, 2.4), (6.0, 0.0), (9.0, -2.4), (12.0, 0.0)], true),
            curve("ParamAngleY", params, &[(0.0, 0.0), (2.8, 2.5), (5.6, 0.0), (8.4, -2.5), (11.2, 0.0), (12.0, 0.0)], true),
            curve("ParamBodyAngleZ", params, &[(0.0, 0.0), (3.0, 2.0), (6.0, 0.0), (9.0, -2.0), (12.0, 0.0)], true),
            curve("ParamBodyAngleY", params, &[(0.0, 0.0), (4.0, 1.2), (8.0, -1.2), (12.0, 0.0)], true),
            curve("ParamArmLA", params, &[(0.0, 0.0), (6.0, 2.5), (12.0, 0.0)], true),
            curve("ParamArmRA", params, &[(0.0, 0.0), (3.0, -1.5), (6.0, 0.0), (9.0, -1.5), (12.0, 0.0)], true),
        ],
    }
}

pub fn wave_motion(params: &Params) -> Clip {
    Clip {
        name: "Wave".into(),
        dur: 2.8,
        looped: false,
        curves: vec![
            curve("ParamArmRA", params, &[(0.0, 0.0), (0.32, -28.0), (0.8, -28.0), (1.05, -8.0), (1.35, -28.0), (1.6, -8.0), (1.85, -28.0), (2.2, -28.0), (2.5, 0.0), (2.8, 0.0)], false),
            curve("ParamAngleZ", params, &[(0.0, 0.0), (0.35, 6.0), (2.2, 6.0), (2.5, 0.0)], false),
            curve("ParamMouthForm", params, &[(0.0, 0.0), (0.4, 0.8), (2.2, 0.8), (2.5, 0.0)], false),
            curve("ParamEyeLSmile", params, &[(0.0, 0.0), (0.4, 0.9), (2.2, 0.9), (2.5, 0.0)], false),
            curve("ParamEyeRSmile", params, &[(0.0, 0.0), (0.4, 0.9), (2.2, 0.9), (2.5, 0.0)], false),
            curve("ParamBrowLY", params, &[(0.0, 0.0), (0.4, 0.5), (2.2, 0.5), (2.5, 0.0)], false),
        ],
    }
}

pub fn hop_motion(params: &Params) -> Clip {
    Clip {
        name: "Hop".into(),
        dur: 1.7,
        looped: false,
        curves: vec![
            curve("ParamBaseY", params, &[(0.0, 0.0), (0.22, -6.0), (0.4, 10.0), (0.62, 4.0), (0.8, 0.0), (0.95, -5.0), (1.2, 0.0), (1.7, 0.0)], false),
            curve("ParamAngleY", params, &[(0.0, 0.0), (0.45, 10.0), (0.9, 0.0)], false),
            curve("ParamEyeBallY", params, &[(0.0, 0.0), (0.45, -0.6), (0.9, 0.0)], false),
            curve("ParamMouthOpenY", params, &[(0.0, 0.0), (0.45, 0.4), (0.9, 0.0)], false),
            curve("ParamBrowLY", params, &[(0.0, 0.0), (0.45, 0.7), (0.9, 0.0)], false),
            curve("ParamBrowRY", params, &[(0.0, 0.0), (0.45, 0.7), (0.9, 0.0)], false),
            curve("ParamAngleZ", params, &[(0.0, 0.0), (0.5, 5.0), (1.1, -3.0), (1.7, 0.0)], false),
        ],
    }
}
