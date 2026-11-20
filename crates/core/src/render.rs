//! Software rasterizer: RGBA framebuffer, textured triangle mesh drawing with
//! bilinear sampling, and soft ellipse fills. No GPU required — this is what
//! makes the same engine run on desktop, wasm and Android.

use crate::math::{clamp, Vec2};
use crate::model::{Part, PartFrame};

pub struct Fb {
    pub w: u32,
    pub h: u32,
    pub buf: Vec<u8>,
}

impl Fb {
    pub fn new(w: u32, h: u32) -> Self {
        Self { w, h, buf: vec![0u8; (w * h * 4) as usize] }
    }

    pub fn clear(&mut self, r: u8, g: u8, b: u8) {
        for px in self.buf.chunks_exact_mut(4) {
            px[0] = r;
            px[1] = g;
            px[2] = b;
            px[3] = 255;
        }
    }

    #[inline]
    fn blend(&mut self, x: i64, y: i64, r: f32, g: f32, b: f32, a: f32) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 || a <= 0.0 {
            return;
        }
        let i = ((y as usize) * self.w as usize + x as usize) * 4;
        let da = self.buf[i + 3] as f32 / 255.0;
        let oa = a + da * (1.0 - a);
        if oa <= 0.0 {
            return;
        }
        let px = &mut self.buf[i..i + 3];
        for c in 0..3 {
            let src = [r, g, b][c];
            let dst = px[c] as f32;
            px[c] = ((1.0 - a) * dst + a * src).clamp(0.0, 255.0) as u8;
        }
        self.buf[i + 3] = (oa * 255.0).clamp(0.0, 255.0) as u8;
    }

    /// Soft ellipse fill (used for the floor shadow).
    pub fn fill_ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, r: u8, g: u8, b: u8, alpha: f32) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let x0 = (cx - rx).floor().max(0.0) as i64;
        let x1 = ((cx + rx).ceil() as i64).min(self.w as i64 - 1);
        let y0 = (cy - ry).floor().max(0.0) as i64;
        let y1 = ((cy + ry).ceil() as i64).min(self.h as i64 - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let nx = (x as f32 + 0.5 - cx) / rx;
                let ny = (y as f32 + 0.5 - cy) / ry;
                let d = (nx * nx + ny * ny).sqrt();
                if d <= 1.0 {
                    let a = alpha * (1.0 - crate::math::smoothstep(0.75, 1.0, d));
                    self.blend(x, y, r as f32, g as f32, b as f32, a.clamp(0.0, 1.0));
                }
            }
        }
    }

    #[inline]
    fn sample_bilinear(tex: &crate::model::Tex, u: f32, v: f32) -> [f32; 4] {
        let tw = tex.w as f32;
        let th = tex.h as f32;
        let x = clamp(u, 0.0, tw - 1.0);
        let y = clamp(v, 0.0, th - 1.0);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(tex.w as usize - 1);
        let y1 = (y0 + 1).min(tex.h as usize - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let at = |xx: usize, yy: usize| -> [f32; 4] {
            let i = (yy * tex.w as usize + xx) * 4;
            [
                tex.data[i] as f32,
                tex.data[i + 1] as f32,
                tex.data[i + 2] as f32,
                tex.data[i + 3] as f32,
            ]
        };
        let c00 = at(x0, y0);
        let c10 = at(x1, y0);
        let c01 = at(x0, y1);
        let c11 = at(x1, y1);
        let mut out = [0f32; 4];
        for c in 0..4 {
            let a = c00[c] * (1.0 - fx) + c10[c] * fx;
            let b = c01[c] * (1.0 - fx) + c11[c] * fx;
            out[c] = a * (1.0 - fy) + b * fy;
        }
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn raster_tri(
        &mut self,
        v0: Vec2,
        v1: Vec2,
        v2: Vec2,
        uv0: [f32; 2],
        uv1: [f32; 2],
        uv2: [f32; 2],
        tex: &crate::model::Tex,
        alpha: f32,
        solid: Option<[u8; 4]>,
    ) {
        let area = (v1.x - v0.x) * (v2.y - v0.y) - (v1.y - v0.y) * (v2.x - v0.x);
        if area.abs() < 0.0001 {
            return;
        }
        let xmin = v0.x.min(v1.x).min(v2.x).floor().max(0.0) as i64;
        let xmax = (v0.x.max(v1.x).max(v2.x).ceil() as i64).min(self.w as i64 - 1);
        let ymin = v0.y.min(v1.y).min(v2.y).floor().max(0.0) as i64;
        let ymax = (v0.y.max(v1.y).max(v2.y).ceil() as i64).min(self.h as i64 - 1);
        if xmin > xmax || ymin > ymax {
            return;
        }
        for y in ymin..=ymax {
            for x in xmin..=xmax {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let w0 = (v1.x - px) * (v2.y - py) - (v1.y - py) * (v2.x - px);
                let w1 = (v2.x - px) * (v0.y - py) - (v2.y - py) * (v0.x - px);
                let w2 = (v0.x - px) * (v1.y - py) - (v0.y - py) * (v1.x - px);
                let inside = if area > 0.0 {
                    w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0
                } else {
                    w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0
                };
                if !inside {
                    continue;
                }
                let b0 = w0 / area;
                let b1 = w1 / area;
                let b2 = w2 / area;
                match solid {
                    Some(c) => {
                        self.blend(x, y, c[0] as f32, c[1] as f32, c[2] as f32, (c[3] as f32 / 255.0) * alpha);
                    }
                    None => {
                        let u = uv0[0] * b0 + uv1[0] * b1 + uv2[0] * b2;
                        let v = uv0[1] * b0 + uv1[1] * b1 + uv2[1] * b2;
                        let c = Self::sample_bilinear(tex, u, v);
                        self.blend(x, y, c[0], c[1], c[2], (c[3] / 255.0) * alpha);
                    }
                }
            }
        }
    }

    /// Draw one part (grid mesh or ellipse fan) from its evaluated frame.
    pub fn draw_part(
        &mut self,
        part: &Part,
        frame: &PartFrame,
        s: f32,
        ox: f32,
        oy: f32,
    ) {
        if frame.alpha <= 0.015 {
            return;
        }
        let to_screen = |v: Vec2| -> Vec2 { Vec2::new(v.x * s + ox, v.y * s + oy) };
        let pos: Vec<Vec2> = frame.pos.iter().map(|v| to_screen(*v)).collect();

        if part.fan {
            // solid procedural color (mouth opening)
            let solid = Some(part.color);
            let n = pos.len() - 1;
            for i in 0..n.saturating_sub(1) {
                self.raster_tri(
                    pos[0], pos[i + 1], pos[i + 2],
                    [0.0, 0.0], [0.0, 0.0], [0.0, 0.0],
                    &part.tex, frame.alpha, solid,
                );
            }
        } else {
            let cols = part.cols;
            let rows = part.rows;
            let idx = |i: usize, j: usize| j * (cols + 1) + i;
            let tex = &part.tex;
            for j in 0..rows {
                for i in 0..cols {
                    let a = idx(i, j);
                    let b = idx(i + 1, j);
                    let c = idx(i, j + 1);
                    let d = idx(i + 1, j + 1);
                    self.raster_tri(pos[a], pos[b], pos[c], part.uv[a], part.uv[b], part.uv[c], tex, frame.alpha, None);
                    self.raster_tri(pos[b], pos[d], pos[c], part.uv[b], part.uv[d], part.uv[c], tex, frame.alpha, None);
                }
            }
        }
    }
}
