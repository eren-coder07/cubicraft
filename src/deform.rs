// CubiCraft — deformer tree.
// Clean-room implementation of the Cubism deformer concept:
//   * Rotation deformers  : affine rotation of everything inside them.
//   * Warp deformers      : an NxM grid of control points; points inside the
//                           rect are mapped with per-cell bilinear interpolation
//                           of the (moving) grid vertices.
//   * Art meshes          : grid-tessellated textured quads ("art meshes").
// Three kinds of deformation combine on every warp grid vertex:
//   * base_offsets    — static, parameter-independent shape edits ("form")
//   * keyforms        — per-parameter offset pairs recorded at norm -1 and +1
//                       (interpolated exactly like Cubism keyforms)
//   * drivers         — parametric drivers (shift / scale / rotate)
// The tree is evaluated leaf -> root, exactly like Cubism's hierarchy.

use macroquad::prelude::*;
use serde::{Deserialize, Serialize};

use crate::params::Params;
use crate::serde_math::{rect, v2, v2vec};

/// Vertex weight function over the unit square (u, v) of a deformer rect.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum Weight {
    One,
    EdgeX(f32),
    EdgeY(f32),
    Bottom(f32),
    Top(f32),
    Left(f32),
    Right(f32),
    VGrad(f32, f32),
    UGrad(f32, f32),
}

impl Weight {
    pub fn eval(self, u: f32, v: f32) -> f32 {
        match self {
            Weight::One => 1.0,
            Weight::EdgeX(k) => 1.0 + k * (2.0 * u - 1.0).abs(),
            Weight::EdgeY(k) => 1.0 + k * (2.0 * v - 1.0).abs(),
            Weight::Bottom(k) => 1.0 + k * v,
            Weight::Top(k) => 1.0 + k * (1.0 - v),
            Weight::Left(k) => 1.0 + k * (1.0 - u),
            Weight::Right(k) => 1.0 + k * u,
            Weight::VGrad(k0, k1) => k0 + k1 * v,
            Weight::UGrad(k0, k1) => k0 + k1 * u,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum DriverKind {
    ShiftX { scale: f32, w: Weight },
    ShiftY { scale: f32, w: Weight },
    ScaleX { cu: f32, k: f32, w: Weight },
    ScaleY { cv: f32, k: f32, w: Weight },
    RotBy {
        #[serde(with = "v2")]
        pivot: Vec2,
        deg: f32,
        w: Weight,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Driver {
    pub pid: usize,
    pub kind: DriverKind,
}

impl Driver {
    pub fn apply(&self, norm: f32, u: f32, v: f32, rect: Rect) -> (f32, f32, f32) {
        match self.kind {
            DriverKind::ShiftX { scale, w } => (scale * norm * w.eval(u, v), 0.0, 0.0),
            DriverKind::ShiftY { scale, w } => (0.0, scale * norm * w.eval(u, v), 0.0),
            DriverKind::ScaleX { cu, k, w } => ((u - cu) * rect.w * k * norm * w.eval(u, v), 0.0, 0.0),
            DriverKind::ScaleY { cv, k, w } => (0.0, (v - cv) * rect.h * k * norm * w.eval(u, v), 0.0),
            DriverKind::RotBy { pivot, deg, w } => (0.0, 0.0, deg * norm * w.eval(u, v)),
        }
    }
}

pub fn apply_rot(p: Vec2, pivot: Vec2, deg: f32) -> Vec2 {
    let r = deg.to_radians();
    let d = p - pivot;
    let c = r.cos();
    let s = r.sin();
    Vec2::new(pivot.x + d.x * c - d.y * s, pivot.y + d.x * s + d.y * c)
}

/// Cubism-style keyform: per-grid-vertex offsets recorded at param norm -1 and +1.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Keyform {
    pub pid: usize,
    #[serde(with = "v2vec")]
    pub pos: Vec<Vec2>,
    #[serde(with = "v2vec")]
    pub neg: Vec<Vec2>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WarpDef {
    #[serde(with = "rect")]
    pub rect: Rect,
    pub div_x: usize,
    pub div_y: usize,
    pub drivers: Vec<Driver>,
    /// static shape edits, one per grid vertex (may be empty)
    #[serde(default, with = "v2vec")]
    pub base_offsets: Vec<Vec2>,
    /// per-parameter keyform offsets
    #[serde(default)]
    pub keyforms: Vec<Keyform>,
}

impl WarpDef {
    pub fn grid_size(&self) -> usize {
        (self.div_x + 1) * (self.div_y + 1)
    }

    /// Accumulated offset per grid vertex (base + keyforms).
    pub fn offsets(&self, params: &Params) -> Vec<Vec2> {
        let n = self.grid_size();
        let mut off = vec![Vec2::ZERO; n];
        for (i, o) in self.base_offsets.iter().enumerate() {
            if i < n {
                off[i] += *o;
            }
        }
        for kf in &self.keyforms {
            let t = ((params.norm(kf.pid) + 1.0) / 2.0).clamp(0.0, 1.0);
            for i in 0..n {
                if i < kf.pos.len() && i < kf.neg.len() {
                    off[i] += kf.neg[i].lerp(kf.pos[i], t);
                }
            }
        }
        off
    }

    pub fn grid_points(&self, params: &Params) -> Vec<Vec2> {
        let (nx, ny) = (self.div_x + 1, self.div_y + 1);
        let offs = self.offsets(params);
        let mut pts = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                let u = i as f32 / self.div_x as f32;
                let v = j as f32 / self.div_y as f32;
                let mut p = Vec2::new(self.rect.x + u * self.rect.w, self.rect.y + v * self.rect.h);
                for d in &self.drivers {
                    let (dx, dy, deg) = d.apply(params.norm(d.pid), u, v, self.rect);
                    if deg != 0.0 {
                        if let DriverKind::RotBy { pivot, .. } = d.kind {
                            p = apply_rot(p, pivot, deg);
                        }
                    }
                    p.x += dx;
                    p.y += dy;
                }
                let k = j * nx + i;
                if k < offs.len() {
                    p += offs[k];
                }
                pts.push(p);
            }
        }
        pts
    }

    /// Map a point in this deformer's own space to the parent's space.
    pub fn map(&self, params: &Params, grid: &[Vec2], p: Vec2) -> Vec2 {
        let x = ((p.x - self.rect.x) / self.rect.w).clamp(0.0, 1.0) * self.div_x as f32;
        let y = ((p.y - self.rect.y) / self.rect.h).clamp(0.0, 1.0) * self.div_y as f32;
        let i = (x.floor() as usize).min(self.div_x - 1);
        let j = (y.floor() as usize).min(self.div_y - 1);
        let u = x - i as f32;
        let v = y - j as f32;
        let idx = |i: usize, j: usize| j * (self.div_x + 1) + i;
        let p00 = grid[idx(i, j)];
        let p10 = grid[idx(i + 1, j)];
        let p01 = grid[idx(i, j + 1)];
        let p11 = grid[idx(i + 1, j + 1)];
        let a = p00.lerp(p10, u);
        let b = p01.lerp(p11, u);
        a.lerp(b, v)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RotDef {
    #[serde(with = "v2")]
    pub pivot: Vec2,
    pub drivers: Vec<Driver>,
}

impl RotDef {
    pub fn angle(&self, params: &Params) -> f32 {
        let mut a = 0.0;
        for d in &self.drivers {
            if let DriverKind::RotBy { deg, w, .. } = d.kind {
                a += deg * params.norm(d.pid) * w.eval(0.5, 0.5);
            }
        }
        a
    }
    pub fn map(&self, params: &Params, p: Vec2) -> Vec2 {
        apply_rot(p, self.pivot, self.angle(params))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshDef {
    #[serde(with = "rect")]
    pub rect: Rect,
    pub div_x: usize,
    pub div_y: usize,
    #[serde(with = "rect")]
    pub uv: Rect,
    /// procedural part name this mesh samples from the atlas
    pub part: String,
    pub drivers: Vec<Driver>,
    pub base_opacity: f32,
    pub opac_drivers: Vec<(usize, f32)>,
}

impl MeshDef {
    pub fn base_points(&self, params: &Params) -> Vec<Vec2> {
        let (nx, ny) = (self.div_x + 1, self.div_y + 1);
        let mut pts = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                let u = i as f32 / self.div_x as f32;
                let v = j as f32 / self.div_y as f32;
                let mut p = Vec2::new(self.rect.x + u * self.rect.w, self.rect.y + v * self.rect.h);
                for d in &self.drivers {
                    let (dx, dy, deg) = d.apply(params.norm(d.pid), u, v, self.rect);
                    if deg != 0.0 {
                        if let DriverKind::RotBy { pivot, .. } = d.kind {
                            p = apply_rot(p, pivot, deg);
                        }
                    }
                    p.x += dx;
                    p.y += dy;
                }
                pts.push(p);
            }
        }
        pts
    }

    pub fn opacity(&self, params: &Params) -> f32 {
        let mut o = self.base_opacity;
        for (pid, k) in &self.opac_drivers {
            o += k * params.norm(*pid);
        }
        o.clamp(0.0, 1.0)
    }

    pub fn uvs(&self, flip_v: bool) -> Vec<Vec2> {
        let (nx, ny) = (self.div_x + 1, self.div_y + 1);
        let mut uvs = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            for i in 0..nx {
                let u = i as f32 / self.div_x as f32;
                let v = j as f32 / self.div_y as f32;
                let mut uu = (self.uv.x + u * self.uv.w) / ATLAS_W as f32;
                let mut vv = (self.uv.y + v * self.uv.h) / ATLAS_H as f32;
                if flip_v {
                    vv = 1.0 - vv;
                }
                uvs.push(Vec2::new(uu, vv));
            }
        }
        uvs
    }

    pub fn indices(&self) -> Vec<u16> {
        let (nx, ny) = (self.div_x + 1, self.div_y + 1);
        let mut idx = Vec::with_capacity(nx * ny * 6);
        for j in 0..ny - 1 {
            for i in 0..nx - 1 {
                let a = (j * nx + i) as u16;
                let b = (j * nx + i + 1) as u16;
                let c = ((j + 1) * nx + i) as u16;
                let d = ((j + 1) * nx + i + 1) as u16;
                idx.extend_from_slice(&[a, b, d, a, d, c]);
            }
        }
        idx
    }
}

pub const ATLAS_W: u32 = 2048;
pub const ATLAS_H: u32 = 2048;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Node {
    Warp { def: WarpDef, children: Vec<Node> },
    Rot { def: RotDef, children: Vec<Node> },
    Mesh(MeshDef),
}

/// One evaluated art mesh for a frame (model-space positions).
pub struct FrameMesh {
    pub verts: Vec<Vec2>,
    pub uvs: Vec<Vec2>,
    pub idx: Vec<u16>,
    pub opacity: f32,
    pub node_id: u32,
}

pub struct WarpFrame {
    pub node_id: u32,
    pub div_x: usize,
    pub div_y: usize,
    /// mapped grid points, in model space
    pub pts: Vec<Vec2>,
}

pub struct RotFrame {
    pub node_id: u32,
    pub pivot: Vec2,
    pub angle_deg: f32,
}

#[derive(Default)]
pub struct EvalResult {
    pub meshes: Vec<FrameMesh>,
    pub warps: Vec<WarpFrame>,
    pub rots: Vec<RotFrame>,
}

impl EvalResult {
    pub fn mesh_bbox(&self, node_id: u32) -> Option<Rect> {
        let mut r: Option<Rect> = None;
        for m in &self.meshes {
            if m.node_id == node_id {
                for v in &m.verts {
                    r = Some(match r {
                        None => Rect::new(v.x, v.y, 0.0, 0.0),
                        Some(b) => {
                            let x0 = b.x.min(v.x);
                            let y0 = b.y.min(v.y);
                            let x1 = (b.x + b.w).max(v.x);
                            let y1 = (b.y + b.h).max(v.y);
                            Rect::new(x0, y0, x1 - x0, y1 - y0)
                        }
                    });
                }
            }
        }
        r
    }

    /// AABB over a warp's mapped grid points
    pub fn warp_bbox(&self, node_id: u32) -> Option<Rect> {
        for w in &self.warps {
            if w.node_id == node_id {
                return Some(points_bbox(&w.pts));
            }
        }
        None
    }
}

pub fn points_bbox(pts: &[Vec2]) -> Rect {
    let mut b = Rect::new(f32::MAX, f32::MAX, 0.0, 0.0);
    for p in pts {
        b.x = b.x.min(p.x);
        b.y = b.y.min(p.y);
        b.w = (b.x + b.w).max(p.x) - b.x;
        b.h = (b.y + b.h).max(p.y) - b.y;
    }
    b
}

type PathStack<'a> = Vec<(&'a Node, Option<Vec<Vec2>>)>;

fn eval_node<'a>(node: &'a Node, path: &mut PathStack<'a>, params: &Params, out: &mut EvalResult, id: &mut u32) {
    let my_id = *id;
    *id += 1;
    match node {
        Node::Warp { def: w, children: children } => {
            let grid = w.grid_points(params);
            let mapped: Vec<Vec2> = grid
                .iter()
                .map(|p| {
                    let mut q = *p;
                    for (anc, g) in path.iter().rev() {
                        q = map_node(anc, g.as_deref(), params, q);
                    }
                    q
                })
                .collect();
            out.warps.push(WarpFrame { node_id: my_id, div_x: w.div_x, div_y: w.div_y, pts: mapped });
            path.push((node, Some(grid)));
            for c in children {
                eval_node(c, path, params, out, id);
            }
            path.pop();
        }
        Node::Rot { def: r, children: children } => {
            let pivot_world = {
                let mut q = r.pivot;
                for (anc, g) in path.iter().rev() {
                    q = map_node(anc, g.as_deref(), params, q);
                }
                q
            };
            out.rots.push(RotFrame { node_id: my_id, pivot: pivot_world, angle_deg: r.angle(params) });
            path.push((node, None));
            for c in children {
                eval_node(c, path, params, out, id);
            }
            path.pop();
        }
        Node::Mesh(m) => {
            let base = m.base_points(params);
            let uvs = m.uvs(FLIP_V);
            let mut world = Vec::with_capacity(base.len());
            for p in &base {
                let mut q = *p;
                for (anc, g) in path.iter().rev() {
                    q = map_node(anc, g.as_deref(), params, q);
                }
                world.push(q);
            }
            out.meshes.push(FrameMesh {
                verts: world,
                uvs,
                idx: m.indices(),
                opacity: m.opacity(params),
                node_id: my_id,
            });
        }
    }
}

fn map_node(node: &Node, grid: Option<&[Vec2]>, params: &Params, p: Vec2) -> Vec2 {
    match node {
        Node::Warp { def: w, children: _ } => {
            let g = grid.expect("warp ancestor must carry its grid");
            w.map(params, g, p)
        }
        Node::Rot { def: r, children: _ } => r.map(params, p),
        Node::Mesh(_) => p,
    }
}

pub fn eval(root: &Node, params: &Params) -> EvalResult {
    let mut out = EvalResult::default();
    let mut id = 0u32;
    let mut path: PathStack = Vec::new();
    eval_node(root, &mut path, params, &mut out, &mut id);
    out
}

// ---------- tree editing helpers ----------

/// Immutable node access by pre-order id.
pub fn node_at<'a>(root: &'a Node, target: u32) -> Option<&'a Node> {
    fn walk<'a>(node: &'a Node, target: u32, counter: &mut u32) -> Option<&'a Node> {
        let my = *counter;
        *counter += 1;
        if my == target {
            return Some(node);
        }
        let children: &'a Vec<Node> = match node {
            Node::Warp { children, .. } | Node::Rot { children, .. } => children,
            Node::Mesh(_) => return None,
        };
        for ch in children.iter() {
            if let Some(found) = walk(ch, target, counter) {
                return Some(found);
            }
        }
        None
    }
    walk(root, target, &mut 0)
}

/// Mutable node access by pre-order id.
pub fn node_at_mut<'a>(root: &'a mut Node, target: u32) -> Option<&'a mut Node> {
    fn walk<'a>(node: &'a mut Node, target: u32, counter: &mut u32) -> Option<&'a mut Node> {
        let my = *counter;
        *counter += 1;
        if my == target {
            return Some(node);
        }
        let children: &'a mut Vec<Node> = match node {
            Node::Warp { children, .. } | Node::Rot { children, .. } => children,
            Node::Mesh(_) => return None,
        };
        for ch in children.iter_mut() {
            if let Some(found) = walk(ch, target, counter) {
                return Some(found);
            }
        }
        None
    }
    walk(root, target, &mut 0)
}

/// The node whose children list contains `target` (pre-order ids).
pub fn parent_of<'a>(root: &'a mut Node, target: u32) -> Option<&'a mut Node> {
    fn walk<'a>(node: &'a mut Node, own_id: u32, target: u32, counter: &mut u32) -> Option<&'a mut Node> {
        // pass 1: find which child contains target (immutable scan)
        let mut hit: Option<(usize, u32)> = None;
        let mut lo = *counter;
        {
            let children: &Vec<Node> = match &*node {
                Node::Warp { children, .. } | Node::Rot { children, .. } => children,
                Node::Mesh(_) => return None,
            };
            for (i, ch) in children.iter().enumerate() {
                let size = subtree_size(ch);
                if target == lo || (target > lo && target < lo + size) {
                    hit = Some((i, lo));
                    break;
                }
                lo += size;
            }
        }
        // pass 2: act on the result (mutable borrow taken only after the check)
        let (i, child_id) = hit?;
        if target == child_id {
            return Some(node);
        }
        *counter = child_id + 1;
        let children: &'a mut Vec<Node> = match node {
            Node::Warp { children, .. } | Node::Rot { children, .. } => children,
            Node::Mesh(_) => return None,
        };
        walk(&mut children[i], child_id, target, counter)
    }
    walk(root, 0, target, &mut 1)
}

/// Pre-order id of the parent of `target`.
pub fn parent_id_of(root: &Node, target: u32) -> Option<u32> {
    fn walk(node: &Node, own_id: u32, target: u32, counter: &mut u32) -> Option<u32> {
        let children: &Vec<Node> = match node {
            Node::Warp { children, .. } | Node::Rot { children, .. } => children,
            Node::Mesh(_) => return None,
        };
        let mut lo = *counter;
        for ch in children.iter() {
            let child_id = lo;
            let size = subtree_size(ch);
            if target == child_id {
                return Some(own_id);
            }
            if target > child_id && target < child_id + size {
                *counter = child_id + 1;
                return walk(ch, child_id, target, counter);
            }
            lo += size;
        }
        *counter = lo;
        None
    }
    walk(root, 0, target, &mut 1)
}

/// Count nodes in subtree (including root of subtree).
pub fn subtree_size(node: &Node) -> u32 {
    match node {
        Node::Mesh(_) => 1,
        Node::Warp { def: _, children: c } | Node::Rot { def: _, children: c } => 1 + c.iter().map(subtree_size).sum::<u32>(),
    }
}

/// Index of the direct child with id `target` inside `parent` (whose own id is `parent_id`).
pub fn child_index(parent: &Node, parent_id: u32, target: u32) -> Option<usize> {
    let (Node::Warp { def: _, children: c } | Node::Rot { def: _, children: c }) = parent else { return None };
    let mut lo = parent_id + 1;
    for (i, ch) in c.iter().enumerate() {
        let size = subtree_size(ch);
        if target == lo {
            return Some(i);
        }
        lo += size;
    }
    None
}

/// Ancestor chain (root -> immediate parent) of a node, immutable.
pub fn ancestors<'a>(root: &'a Node, target: u32) -> Vec<&'a Node> {
    fn walk<'a>(node: &'a Node, target: u32, counter: &mut u32, path: &mut Vec<&'a Node>) -> bool {
        let my = *counter;
        *counter += 1;
        if my == target {
            return true;
        }
        match node {
            Node::Warp { def: _, children: c } | Node::Rot { def: _, children: c } => {
                path.push(node);
                for ch in c {
                    if walk(ch, target, counter, path) {
                        return true;
                    }
                }
                path.pop();
                false
            }
            Node::Mesh(_) => false,
        }
    }
    let mut c = 0u32;
    let mut path = Vec::new();
    let _ = walk(root, target, &mut c, &mut path);
    path
}

/// Map a point from a node's own space up to model space through its ancestors.
pub fn map_point_to_world(path: &[&Node], params: &Params, mut p: Vec2) -> Vec2 {
    for anc in path.iter().rev() {
        match anc {
            Node::Warp { def: w, children: _ } => {
                let g = w.grid_points(params);
                p = w.map(params, &g, p);
            }
            Node::Rot { def: r, children: _ } => p = r.map(params, p),
            Node::Mesh(_) => {}
        }
    }
    p
}

/// Convert a world-space delta to a delta in a node's own space,
/// via a numeric 2x2 Jacobian of the ancestor chain at the given local point.
pub fn world_delta_to_local(path: &[&Node], params: &Params, local_at: Vec2, world_delta: Vec2) -> Vec2 {
    let e = 0.01;
    let p0 = map_point_to_world(path, params, local_at);
    let px = map_point_to_world(path, params, local_at + Vec2::new(e, 0.0));
    let py = map_point_to_world(path, params, local_at + Vec2::new(0.0, e));
    let j11 = (px.x - p0.x) / e;
    let j21 = (px.y - p0.y) / e;
    let j12 = (py.x - p0.x) / e;
    let j22 = (py.y - p0.y) / e;
    let det = j11 * j22 - j12 * j21;
    if det.abs() < 1e-8 {
        return world_delta;
    }
    // inverse of [[j11, j12], [j21, j22]]
    let ix = (j22 * world_delta.x - j12 * world_delta.y) / det;
    let iy = (-j21 * world_delta.x + j11 * world_delta.y) / det;
    Vec2::new(ix, iy)
}

/// Map a world-space point down INTO a node's own space via numeric inversion.
pub fn world_point_to_local(path: &[&Node], params: &Params, world: Vec2) -> Vec2 {
    // start from a rough guess and iterate (Newton with numeric jacobian)
    let mut local = Vec2::new(1.5, 1.0);
    for _ in 0..8 {
        let e = 0.01;
        let p0 = map_point_to_world(path, params, local);
        let px = map_point_to_world(path, params, local + Vec2::new(e, 0.0));
        let py = map_point_to_world(path, params, local + Vec2::new(0.0, e));
        let j11 = (px.x - p0.x) / e;
        let j21 = (px.y - p0.y) / e;
        let j12 = (py.x - p0.x) / e;
        let j22 = (py.y - p0.y) / e;
        let det = j11 * j22 - j12 * j21;
        if det.abs() < 1e-8 {
            break;
        }
        let dx = world.x - p0.x;
        let dy = world.y - p0.y;
        let ix = (j22 * dx - j12 * dy) / det;
        let iy = (-j21 * dx + j11 * dy) / det;
        local += vec2(ix, iy).clamp(vec2(-0.5, -0.5), vec2(0.5, 0.5));
    }
    local
}

// ---------- rig tree UI info ----------

#[derive(Clone)]
pub struct TreeNodeInfo {
    pub id: u32,
    pub label: String,
    pub kind: &'static str,
    pub depth: usize,
}

pub fn collect_tree(node: &Node, out: &mut Vec<TreeNodeInfo>) {
    let mut id = 0u32;
    fn walk(node: &Node, depth: usize, out: &mut Vec<TreeNodeInfo>, id: &mut u32) {
        let my = *id;
        *id += 1;
        match node {
            Node::Warp { def: w, children: c } => {
                out.push(TreeNodeInfo { id: my, label: format!("warp {}x{}", w.div_x, w.div_y), kind: "W", depth });
                for ch in c {
                    walk(ch, depth + 1, out, id);
                }
            }
            Node::Rot { def: _, children: c } => {
                out.push(TreeNodeInfo { id: my, label: "rotate".into(), kind: "R", depth });
                for ch in c {
                    walk(ch, depth + 1, out, id);
                }
            }
            Node::Mesh(m) => {
                out.push(TreeNodeInfo { id: my, label: format!("mesh «{}»", m.part), kind: "M", depth });
            }
        }
    }
    walk(node, 0, out, &mut id);
}

pub const FLIP_V: bool = true;
