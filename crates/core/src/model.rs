//! The rigged model: texture parts, deformable mesh grids and per-frame
//! evaluation through the deformer chain (local morph -> head chain ->
//! body chain -> root drag).

use crate::math::{clamp, Vec2};
use crate::psd::Psd;
use crate::rig::Spec;
use crate::rig::Params;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Body,
    Head,
    Eye,
    EyeL,
    EyeR,
    EyeLid,
    Mouth,
    MouthOpen,
    Hair,
}

#[derive(Debug, Clone)]
pub struct Tex {
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Part {
    pub name: String,
    pub kind: Kind,
    pub tex: Tex,
    /// Canvas coordinates of the texture's top-left corner.
    pub origin: Vec2,
    /// Base mesh vertices in canvas coordinates.
    pub base: Vec<Vec2>,
    /// UV per vertex in texel coordinates.
    pub uv: Vec<[f32; 2]>,
    pub cols: usize,
    pub rows: usize,
    /// Ellipse fan layout instead of a grid.
    pub fan: bool,
    pub pivot: Vec2,
    pub depth: f32,
    pub rx: f32,
    pub ry: f32,
    pub color: [u8; 4],
    /// Per-frame alpha multiplier.
    pub alpha_scale: f32,
}

#[derive(Debug, Clone)]
pub struct PartFrame {
    pub pos: Vec<Vec2>,
    pub alpha: f32,
}

pub struct Model {
    pub parts: Vec<Part>,
    pub params: Params,
    pub neck: Vec2,
    pub hip: Vec2,
    pub canvas: Vec2,
    pub hair_top: f32,
    pub hair_h: f32,
    frames: Vec<PartFrame>,
}

fn tex_from(w: u32, h: u32, data: Vec<u8>) -> Tex {
    Tex { w, h, data }
}

fn crop_tex(tex: &Tex, rect: [f32; 4]) -> Tex {
    let x0 = rect[0].max(0.0) as u32;
    let y0 = rect[1].max(0.0) as u32;
    let x1 = (rect[2].min(tex.w as f32)) as u32;
    let y1 = (rect[3].min(tex.h as f32)) as u32;
    let w = x1.saturating_sub(x0).max(1);
    let h = y1.saturating_sub(y0).max(1);
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let sy = y0 + y;
        for x in 0..w {
            let sx = x0 + x;
            let si = ((sy * tex.w + sx) * 4) as usize;
            let di = ((y * w + x) * 4) as usize;
            data[di..di + 4].copy_from_slice(&tex.data[si..si + 4]);
        }
    }
    Tex { w, h, data }
}

/// Fill a rect (in tex-local coords) after cutting out features. If opaque
/// skin surrounds the rect on both sides, blend a skin gradient; otherwise
/// make the rect transparent so whatever is drawn underneath (e.g. body skin)
/// shows through during animation.
fn inpaint_rect(tex: &mut Tex, rect: [f32; 4]) {
    let w = tex.w as usize;
    let h = tex.h as usize;
    let x0 = rect[0].round().max(0.0) as usize;
    let y0 = rect[1].round().max(0.0) as usize;
    let x1 = (rect[2].round() as usize).min(w);
    let y1 = (rect[3].round() as usize).min(h);
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    // precompute, per row, the nearest opaque colors to the left and right of
    // the rect (borrowing ends before we mutate)
    let mut fills: Vec<[Option<[f32; 3]>; 2]> = Vec::with_capacity(y1 - y0);
    {
        let data = &tex.data;
        for y in y0..y1 {
            let sample = |x: usize| -> [f32; 3] {
                let i = (y * w + x) * 4;
                [data[i] as f32, data[i + 1] as f32, data[i + 2] as f32]
            };
            let mut left: Option<[f32; 3]> = None;
            let mut x = x0;
            while x > 0 && (x0 - x) < 24 {
                let i = (y * w + x - 1) * 4;
                if data[i + 3] > 128 {
                    left = Some(sample(x - 1));
                    break;
                }
                x -= 1;
            }
            let mut right: Option<[f32; 3]> = None;
            let mut x = x1;
            while x + 1 < w && (x - x1) < 24 {
                let i = (y * w + x + 1) * 4;
                if data[i + 3] > 128 {
                    right = Some(sample(x + 1));
                    break;
                }
                x += 1;
            }
            fills.push([left, right]);
        }
    }
    for (row, y) in (y0..y1).enumerate() {
        let [left, right] = fills[row];
        let n = (x1 - x0) as f32;
        for (k, x) in (x0..x1).enumerate() {
            let t = k as f32 / n.max(1.0);
            let i = (y * w + x) * 4;
            if let (Some(l), Some(r)) = (left, right) {
                // Only paint a skin gradient when both border samples agree
                // (both truly surrounding skin). If they disagree (e.g. one is
                // a dark outline stroke), fall back to transparent fill.
                let close = (0..3).all(|c| (l[c] - r[c]).abs() < 40.0);
                if close {
                    for c in 0..3 {
                        tex.data[i + c] = crate::math::lerp(l[c], r[c], t) as u8;
                    }
                    tex.data[i + 3] = 255;
                } else {
                    tex.data[i + 3] = 0;
                }
            } else {
                // transparent fill — underlying layers show through
                tex.data[i + 3] = 0;
            }
        }
    }
}

impl Model {
    pub fn build(psd: &Psd, spec: &Spec) -> Result<Model, String> {
        let mut parts: Vec<Part> = Vec::with_capacity(spec.parts.len());

        for sp in &spec.parts {
            let kind = match sp.kind.as_str() {
                "body" => Kind::Body,
                "head" => Kind::Head,
                "eye" => match sp.side.as_str() {
                    "L" => Kind::EyeL,
                    _ => Kind::EyeR,
                },
                "lid" => Kind::EyeLid,
                "mouth" => Kind::Mouth,
                "mouth_open" => Kind::MouthOpen,
                "hair" => Kind::Hair,
                other => return Err(format!("unknown part kind {other}")),
            };

            let (tex, origin) = if !sp.from.is_empty() {
                // crop from an existing part
                let src = parts
                    .iter()
                    .find(|p| p.name == sp.from)
                    .ok_or_else(|| format!("part {} references missing part {}", sp.name, sp.from))?;
                let t = crop_tex(&src.tex, sp.rect);
                let o = src.origin.add(Vec2::new(sp.rect[0], sp.rect[1]));
                (t, o)
            } else if let Some(color) = sp.color {
                (Tex { w: 1, h: 1, data: vec![color[0], color[1], color[2], color[3]] }, Vec2::new(0.0, 0.0))
            } else {
                let (data, x0, y0, w, h) = psd.compose(
                    &sp.layers.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                )?;
                (tex_from(w, h, data), Vec2::new(x0 as f32, y0 as f32))
            };

            let pivot = if !sp.from.is_empty() {
                // pivot given in source-part-local coords in the spec -> canvas
                let src = parts
                    .iter()
                    .find(|p| p.name == sp.from)
                    .ok_or_else(|| format!("missing part {}", sp.from))?;
                src.origin.add(Vec2::new(sp.pivot[0], sp.pivot[1]))
            } else {
                Vec2::new(sp.pivot[0], sp.pivot[1])
            };

            let (base, uv, fan, cols, rows) = if kind == Kind::MouthOpen {
                // ellipse fan: vertex 0 = center, then 24 ring points
                let n = 24usize;
                let mut base = Vec::with_capacity(n + 2);
                let mut uv = Vec::with_capacity(n + 2);
                base.push(pivot);
                uv.push([0.0, 0.0]);
                for i in 0..=n {
                    let a = (i as f32 / n as f32) * std::f32::consts::TAU;
                    base.push(Vec2::new(pivot.x + a.cos() * sp.rx, pivot.y + a.sin() * sp.ry));
                    uv.push([0.5, 0.5]);
                }
                (base, uv, true, 0, 0)
            } else {
                let cols = sp.grid[0].max(1);
                let rows = sp.grid[1].max(1);
                let tw = tex.w as f32;
                let th = tex.h as f32;
                let mut base = Vec::with_capacity((cols + 1) * (rows + 1));
                let mut uv = Vec::with_capacity((cols + 1) * (rows + 1));
                for j in 0..=rows {
                    for i in 0..=cols {
                        let x = origin.x + tw * (i as f32 / cols as f32);
                        let y = origin.y + th * (j as f32 / rows as f32);
                        base.push(Vec2::new(x, y));
                        uv.push([
                            (i as f32 / cols as f32) * (tw - 1.0),
                            (j as f32 / rows as f32) * (th - 1.0),
                        ]);
                    }
                }
                (base, uv, false, cols, rows)
            };

            parts.push(Part {
                name: sp.name.clone(),
                kind,
                tex,
                origin,
                base,
                uv,
                cols,
                rows,
                fan,
                pivot,
                depth: sp.depth,
                rx: sp.rx,
                ry: sp.ry,
                color: sp.color.unwrap_or([255, 255, 255, 255]),
                alpha_scale: 1.0,
            });
        }

        // Erase cut-out features (eyes, mouth) from the source face texture.
        for sp in &spec.parts {
            if !sp.from.is_empty() && (sp.kind == "eye" || sp.kind == "mouth") {
                if let Some(src) = parts.iter_mut().find(|p| p.name == sp.from) {
                    inpaint_rect(&mut src.tex, sp.rect);
                }
            }
        }

        let hair_top = parts
            .iter()
            .find(|p| p.kind == Kind::Hair)
            .map(|p| p.origin.y)
            .unwrap_or(0.0);
        let hair_h = parts
            .iter()
            .find(|p| p.kind == Kind::Hair)
            .map(|p| p.tex.h as f32)
            .unwrap_or(1.0);

        let frames = parts
            .iter()
            .map(|p| PartFrame { pos: p.base.clone(), alpha: 1.0 })
            .collect();

        Ok(Model {
            parts,
            params: Params::new(),
            neck: Vec2::new(spec.neck_pivot[0], spec.neck_pivot[1]),
            hip: Vec2::new(spec.hip_pivot[0], spec.hip_pivot[1]),
            canvas: Vec2::new(spec.canvas[0], spec.canvas[1]),
            hair_top,
            hair_h,
            frames,
        })
    }

    /// Evaluate the whole deformer chain for the current parameter values.
    pub fn eval(&mut self) {
        let p = &self.params;
        let ax = p.get("AngleX");
        let ay = p.get("AngleY");
        let az = p.get("AngleZ");
        let ebx = p.get("EyeBallX");
        let eby = p.get("EyeBallY");
        let mouth_open = p.get("MouthOpenY");
        let body_rot = p.get("BodyAngleZ").to_radians() * 0.6;
        let body_dx = p.get("BodyAngleX") * 0.8;
        let body_dy = p.get("BodyAngleY") * 1.4;
        let drag = Vec2::new(p.get("DragX"), p.get("DragY"));
        let sway = p.get("HairSway");
        let sway2 = p.get("HairSway2");

        let head_rot = az.to_radians() * 0.8;
        let head_shift_x = (ax / 30.0) * 16.0;
        let head_shift_y = (ay / 30.0) * 12.0;

        let frames = &mut self.frames;
        for (idx, part) in self.parts.iter().enumerate() {
            let f = &mut frames[idx];
            f.alpha = part.alpha_scale;
            let mut pos: Vec<Vec2> = part.base.clone();

            match part.kind {
                Kind::EyeL | Kind::EyeR => {
                    let id = if part.kind == Kind::EyeL { "EyeLOpen" } else { "EyeROpen" };
                    let open = clamp(self.params.get(id), 0.0, 1.0);
                    let pv = part.pivot;
                    for v in pos.iter_mut() {
                        v.y = pv.y + (v.y - pv.y) * open.max(0.32);
                        v.x += ebx * 3.0;
                        v.y += eby * 2.0;
                    }
                    // fade the eye out as it closes; the procedural lid takes over
                    f.alpha *= 0.03 + 0.97 * open;
                }
                Kind::EyeLid => {
                    let id = if part.name.ends_with("_L") { "EyeLOpen" } else { "EyeROpen" };
                    let open = clamp(self.params.get(id), 0.0, 1.0);
                    f.alpha *= (1.0 - open) * (1.0 - open);
                }
                Kind::Mouth => {
                    f.alpha *= 1.0 - mouth_open * 0.92;
                }
                Kind::MouthOpen => {
                    f.alpha *= mouth_open;
                    let pv = part.pivot;
                    let grow = clamp(mouth_open, 0.0, 1.0);
                    // vertices are center + ring; squash ring vertically by opening
                    for (i, v) in pos.iter_mut().enumerate() {
                        if i == 0 {
                            continue;
                        }
                        let dy = v.y - pv.y;
                        v.y = pv.y + dy * clamp(0.15 + 0.85 * grow, 0.15, 1.0);
                    }
                }
                Kind::Hair => {
                    for v in pos.iter_mut() {
                        let w = clamp((v.y - self.hair_top) / self.hair_h.max(1.0), 0.0, 1.0);
                        let w = w * w * (3.0 - 2.0 * w); // smoothstep weighting
                        v.x += sway * w + sway2 * w * 0.45;
                        v.y += sway.abs() * 0.06 * w;
                    }
                }
                _ => {}
            }

            // head chain (everything except the body)
            if part.kind != Kind::Body {
                let d = part.depth;
                let dx = head_shift_x * d;
                let dy = head_shift_y * d;
                let sx = 1.0 - (ax.abs() / 30.0) * 0.06 * d;
                let rot = head_rot * (0.8 + 0.2 * d);
                let pv = self.neck;
                for v in pos.iter_mut() {
                    let mut q = v.sub(pv).rotate(rot);
                    q.x *= sx;
                    *v = pv.add(q).add(Vec2::new(dx, dy));
                }
            }

            // body chain (whole figure rides on the hips)
            let pv = self.hip;
            for v in pos.iter_mut() {
                let q = v.sub(pv).rotate(body_rot);
                *v = pv.add(q).add(Vec2::new(body_dx, body_dy));
            }

            // root drag
            if drag.x != 0.0 || drag.y != 0.0 {
                for v in pos.iter_mut() {
                    *v = v.add(drag);
                }
            }

            f.pos = pos;
        }
    }

    pub fn frames(&self) -> &[PartFrame] {
        &self.frames
    }

    pub fn part(&self, i: usize) -> &Part {
        &self.parts[i]
    }

    pub fn part_count(&self) -> usize {
        self.parts.len()
    }

    /// Canvas point for the mouth center (used to place procedural mouth).
    pub fn mouth_pivot_screen(&self) -> Vec2 {
        self.parts
            .iter()
            .find(|p| p.kind == Kind::MouthOpen && p.name == "mouth_open")
            .map(|p| p.pivot)
            .unwrap_or(self.canvas.scale(0.5))
    }
}
