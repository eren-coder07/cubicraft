// CubiCraft — editor.
// A Cubism-Editor-style modeling application around the runtime:
// menus, Edit Levels, Part/Deformer panels, Inspector, Parameter palette,
// canvas selection + gizmos, warp-grid keyform editing, draw order,
// parameter creation, clip recording, and .ccm3 project save/load.

use macroquad::prelude::*;
use std::collections::HashMap;

use crate::deform::*;
use crate::motion::{Clip, Curve};
use crate::params::Params;
use crate::project::{self, Project};
use crate::ui;

pub const MENUS: [&str; 6] = ["File", "Edit", "Show", "Modeling", "Animation", "Help"];

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    None,
    NewProject,
    OpenProject,
    SaveProject,
    ReloadMomi,
    Deselect,
    ToggleGrid,
    ToggleMesh,
    TogglePhysics,
    ToggleBlink,
    ToggleLook,
    AddWarp,
    AddRot,
    DeleteSel,
    MoveUp,
    MoveDown,
    RecordStart,
    RecordStop,
    PlayRecorded,
    ClearRecorded,
    About,
}

#[derive(Clone)]
enum Drag {
    Node { grab: Vec2, orig: OrigPose },
    GridPoint { warp_id: u32, idx: usize },
}

#[derive(Clone)]
enum OrigPose {
    MeshRect(Rect),
    WarpRect(Rect),
    Pivot(Vec2),
}

pub struct ParamAdd {
    pub name: String,
    pub group_idx: usize,
    pub min: f32,
    pub def: f32,
    pub max: f32,
}

pub struct Record {
    pub start: f64,
    pub last: f32,
    pub curves: HashMap<usize, Vec<(f32, f32)>>,
}

pub struct Editor {
    pub selected: Option<u32>,
    pub edit_level: usize, // 1 view+params, 2 select+move, 3 + grid/keyform editing
    pub edit_pid: Option<usize>,
    pub drag: Option<Drag>,
    pub menu_open: Option<usize>,
    pub show_grid: bool,
    pub show_mesh: bool,
    pub physics_on: bool,
    pub blink_on: bool,
    pub look_on: bool,
    pub pending_action: Option<Action>,
    pub toast: Option<(String, f64)>,
    pub about: bool,
    pub param_add: Option<ParamAdd>,
    pub record: Option<Record>,
    pub recorded: Option<Clip>,
    pub driver_kind: usize,
    pub driver_weight: usize,
    pub driver_scale: f32,
    pub driver_param: usize,
    pub hover_node: Option<u32>,
    pub tree: Vec<TreeNodeInfo>,
    pub tree_scroll: [f32; 2],
    pub zoom_display: f32,
}

impl Editor {
    pub fn new() -> Self {
        Editor {
            selected: None,
            edit_level: 2,
            edit_pid: None,
            drag: None,
            menu_open: None,
            show_grid: false,
            show_mesh: false,
            physics_on: true,
            blink_on: true,
            look_on: true,
            pending_action: None,
            toast: None,
            about: false,
            param_add: None,
            record: None,
            recorded: None,
            driver_kind: 0,
            driver_weight: 0,
            driver_scale: 0.05,
            driver_param: 14,
            hover_node: None,
            tree: vec![],
            tree_scroll: [0.0; 2],
            zoom_display: 100.0,
        }
    }

    pub fn refresh_tree(&mut self, root: &Node) {
        self.tree.clear();
        collect_tree(root, &mut self.tree);
    }

    pub fn toast(&mut self, msg: &str) {
        self.toast = Some((msg.to_string(), get_time() + 3.5));
    }

    // ---------- menus ----------

    pub fn menu_items(menu: usize) -> &'static [(&'static str, Action)] {
        match menu {
            0 => &[
                ("New project", Action::NewProject),
                ("Open project (.ccm3)…", Action::OpenProject),
                ("Save project (.ccm3)", Action::SaveProject),
                ("Reload Momi (built-in)", Action::ReloadMomi),
            ],
            1 => &[("Deselect", Action::Deselect)],
            2 => &[
                ("Warp grids overlay", Action::ToggleGrid),
                ("Art mesh overlay", Action::ToggleMesh),
                ("Physics", Action::TogglePhysics),
                ("Eye blink", Action::ToggleBlink),
                ("Look-at tracking", Action::ToggleLook),
            ],
            3 => &[
                ("Add warp deformer around selection", Action::AddWarp),
                ("Add rotation deformer around selection", Action::AddRot),
                ("Delete selected", Action::DeleteSel),
                ("Move up (draw order)", Action::MoveUp),
                ("Move down (draw order)", Action::MoveDown),
            ],
            4 => &[
                ("Record clip (6 s)", Action::RecordStart),
                ("Stop recording", Action::RecordStop),
                ("Play recorded clip", Action::PlayRecorded),
                ("Clear recorded clip", Action::ClearRecorded),
            ],
            _ => &[("About CubiCraft", Action::About)],
        }
    }

    pub fn run_action(&mut self, action: Action, model: &mut crate::Model, tap_player: &mut crate::motion::MotionPlayer) {
        let params = &mut model.params;
        match action {
            Action::None => {}
            Action::NewProject => {
                model.root = Node::Warp {
                    def: WarpDef {
                        rect: Rect::new(0.0, 0.0, 3.0, 3.0),
                        div_x: 2,
                        div_y: 2,
                        drivers: vec![],
                        base_offsets: vec![],
                        keyforms: vec![],
                    },
                    children: vec![],
                };
                model.physics_settings.clear();
                model.clips.clear();
                self.selected = None;
                self.refresh_tree(&model.root);
                self.toast("New project — an empty root warp");
            }
            Action::OpenProject => {
                #[cfg(target_arch = "wasm32")]
                {
                    self.toast("File dialogs need the desktop build — copy a .ccm3 into the project folder");
                }
                #[cfg(not(target_arch = "wasm32"))]
                match project::load_file("cubicraft.ccm3") {
                    Ok(p) => {
                        p.install_params(params);
                        let n_clips = p.clips.len();
                        let pname = p.name.clone();
                        model.root = p.root;
                        model.physics_settings = p.physics;
                        model.clips = p.clips;
                        model.rebuild_physics();
                        self.selected = None;
                        self.refresh_tree(&model.root);
                        self.toast(&format!("Opened «{}» ({} clips)", pname, n_clips));
                    }
                    Err(e) => self.toast(&e),
                }
            }
            Action::SaveProject => {
                let proj = Project {
                    format: "ccm3".into(),
                    version: 1,
                    name: model.name.clone(),
                    custom_params: params.defs[project::BASE_PARAM_COUNT..].to_vec(),
                    root: model.root.clone(),
                    physics: model.physics_settings.clone(),
                    clips: model.clips.clone(),
                };
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = proj;
                    self.toast("Saving to disk needs the desktop build (web has no file dialogs)");
                }
                #[cfg(not(target_arch = "wasm32"))]
                match project::save_file(&proj, "cubicraft.ccm3") {
                    Ok(_) => self.toast("Saved cubicraft.ccm3 (open JSON format)"),
                    Err(e) => self.toast(&e),
                }
            }
            Action::ReloadMomi => {
                model.load_momi();
                self.selected = None;
                self.refresh_tree(&model.root);
                self.toast("Momi rebuilt from the procedural source");
            }
            Action::Deselect => {
                self.selected = None;
                self.edit_pid = None;
            }
            Action::ToggleGrid => self.show_grid = !self.show_grid,
            Action::ToggleMesh => self.show_mesh = !self.show_mesh,
            Action::TogglePhysics => self.physics_on = !self.physics_on,
            Action::ToggleBlink => self.blink_on = !self.blink_on,
            Action::ToggleLook => self.look_on = !self.look_on,
            Action::AddWarp => self.wrap_selected(model, true),
            Action::AddRot => self.wrap_selected(model, false),
            Action::DeleteSel => self.delete_selected(model),
            Action::MoveUp => self.reorder_selected(model, -1),
            Action::MoveDown => self.reorder_selected(model, 1),
            Action::RecordStart => {
                if self.record.is_none() {
                    self.record = Some(Record { start: get_time(), last: -1.0, curves: HashMap::new() });
                    self.toast("Recording all parameters for 6 s…");
                }
            }
            Action::RecordStop => {
                if let Some(rec) = self.record.take() {
                    let dur = (get_time() - rec.start) as f32;
                    let mut curves = vec![];
                    let mut pids: Vec<usize> = rec.curves.keys().copied().collect();
                    pids.sort();
                    for pid in pids {
                        let mut keys: Vec<(f32, f32)> = rec.curves[&pid].clone();
                        keys.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                        curves.push(Curve {
                            pid,
                            keys: keys.into_iter().map(|(t, v)| crate::motion::Key { t, v }).collect(),
                            add: false,
                        });
                    }
                    self.recorded = Some(Clip { name: "Recorded".into(), dur: dur.max(0.2), looped: false, curves });
                    self.toast(&format!("Clip recorded ({:.1} s) — Animation ▸ Play", dur));
                }
            }
            Action::PlayRecorded => match &self.recorded {
                Some(clip) => {
                    tap_player.play(clip.clone());
                    self.toast("Playing recorded clip");
                }
                None => self.toast("No recorded clip yet — Animation ▸ Record"),
            },
            Action::ClearRecorded => {
                self.recorded = None;
                self.toast("Recorded clip cleared");
            }
            Action::About => self.about = true,
        }
    }

    fn wrap_selected(&mut self, model: &mut crate::Model, warp: bool) {
        let Some(sel) = self.selected else {
            self.toast("Select something first (Edit Level 2/3)");
            return;
        };
        if sel == 0 {
            self.toast("Cannot wrap the root");
            return;
        }
        let Some(parent_id) = parent_id_of(&model.root, sel) else { return };
        // gather geometry before mutable juggling
        let (sel_rect, _kind) = {
            let s = node_at_mut(&mut model.root, sel).unwrap();
            match s {
                Node::Mesh(m) => (m.rect, "mesh"),
                Node::Warp { def, .. } => (def.rect, "warp"),
                Node::Rot { def, .. } => (Rect::new(def.pivot.x - 0.1, def.pivot.y - 0.1, 0.2, 0.2), "rot"),
            }
        };
        let idx = {
            let parent = node_at_mut(&mut model.root, parent_id).unwrap();
            child_index(&*parent, parent_id, sel).expect("child index")
        };
        // extract selected
        let extracted = {
            let parent = node_at_mut(&mut model.root, parent_id).unwrap();
            let (Node::Warp { children, .. } | Node::Rot { children, .. }) = parent else { return };
            let dummy = Node::Mesh(MeshDef {
                rect: Rect::new(0.0, 0.0, 0.001, 0.001),
                div_x: 1,
                div_y: 1,
                uv: Rect::new(0.0, 0.0, 4.0, 4.0),
                part: String::new(),
                drivers: vec![],
                base_opacity: 0.0,
                opac_drivers: vec![],
            });
            std::mem::replace(&mut children[idx], dummy)
        };
        let new_node = if warp {
            let r = Rect::new(sel_rect.x - 0.06, sel_rect.y - 0.06, sel_rect.w + 0.12, sel_rect.h + 0.12);
            Node::Warp {
                def: WarpDef { rect: r, div_x: 3, div_y: 3, drivers: vec![], base_offsets: vec![], keyforms: vec![] },
                children: vec![extracted],
            }
        } else {
            let pv = vec2(sel_rect.x + sel_rect.w / 2.0, sel_rect.y + sel_rect.h / 2.0);
            Node::Rot { def: RotDef { pivot: pv, drivers: vec![] }, children: vec![extracted] }
        };
        let parent = node_at_mut(&mut model.root, parent_id).unwrap();
        let (Node::Warp { children, .. } | Node::Rot { children, .. }) = parent else { return };
        children[idx] = new_node;
        self.selected = None; // ids shifted; refresh
        self.refresh_tree(&model.root);
        self.toast(if warp { "Warp deformer added around selection" } else { "Rotation deformer added around selection" });
    }

    fn delete_selected(&mut self, model: &mut crate::Model) {
        let Some(sel) = self.selected else { return };
        if sel == 0 {
            self.toast("Cannot delete the root");
            return;
        }
        let Some(parent_id) = parent_id_of(&model.root, sel) else { return };
        let idx = {
            let parent = node_at_mut(&mut model.root, parent_id).unwrap();
            child_index(&*parent, parent_id, sel).expect("child index")
        };
        let parent = node_at_mut(&mut model.root, parent_id).unwrap();
        let (Node::Warp { children, .. } | Node::Rot { children, .. }) = parent else { return };
        children.remove(idx);
        self.selected = None;
        self.refresh_tree(&model.root);
        self.toast("Deleted (node ids shifted — selection cleared)");
    }

    fn reorder_selected(&mut self, model: &mut crate::Model, dir: i32) {
        let Some(sel) = self.selected else { return };
        let Some(parent_id) = parent_id_of(&model.root, sel) else { return };
        let idx = {
            let parent = node_at_mut(&mut model.root, parent_id).unwrap();
            child_index(&*parent, parent_id, sel).expect("child index")
        };
        let parent = node_at_mut(&mut model.root, parent_id).unwrap();
        let (Node::Warp { children, .. } | Node::Rot { children, .. }) = parent else { return };
        let n = children.len();
        let new_idx = (idx as i32 + dir).clamp(0, n as i32 - 1) as usize;
        let item = children.remove(idx);
        children.insert(new_idx, item);
        self.selected = None;
        self.refresh_tree(&model.root);
        self.toast("Draw order changed");
    }

    // ---------- canvas interaction ----------

    pub fn canvas_input(&mut self, model: &mut crate::Model, ev: &EvalResult, ppu: f32, ox: f32, oy: f32, canvas: Rect) {
        let m = mouse_position();
        // only interact inside the canvas viewport
        if !(m.0 >= canvas.x && m.0 <= canvas.x + canvas.w && m.1 >= canvas.y && m.1 <= canvas.y + canvas.h) {
            if is_mouse_button_released(MouseButton::Left) {
                self.drag = None;
            }
            return;
        }
        let world = vec2((m.0 - ox) / ppu, (m.1 - oy) / ppu);
        let pressed = is_mouse_button_pressed(MouseButton::Left);
        let down = is_mouse_button_down(MouseButton::Left);
        let released = is_mouse_button_released(MouseButton::Left);
        let root = &mut model.root;
        let params = &model.params;

        // hover
        self.hover_node = if self.edit_level >= 2 { pick_node(ev, world, root, params) } else { None };

        if released {
            self.drag = None;
        }

        if pressed && self.edit_level >= 2 {
            // start drag: grid point (level 3 + warp selected) or node move
            if self.edit_level >= 3 {
                if let Some(sel) = self.selected {
                    if matches!(node_at_mut(root, sel), Some(Node::Warp { .. })) {
                        if let Some(wf) = ev.warps.iter().find(|w| w.node_id == sel) {
                            let mut best = (usize::MAX, 1e9f32);
                            for (i, p) in wf.pts.iter().enumerate() {
                                let d = world.distance(*p);
                                if d < best.1 {
                                    best = (i, d);
                                }
                            }
                            if best.0 != usize::MAX && best.1 < 0.07 {
                                self.drag = Some(Drag::GridPoint { warp_id: sel, idx: best.0 });
                                return;
                            }
                        }
                    }
                }
            }
            if let Some(sel) = pick_node(ev, world, root, params) {
                self.selected = Some(sel);
                self.drag = Some(Drag::Node {
                    grab: world,
                    orig: match node_at_mut(root, sel).unwrap() {
                        Node::Mesh(m) => OrigPose::MeshRect(m.rect),
                        Node::Warp { def, .. } => OrigPose::WarpRect(def.rect),
                        Node::Rot { def, .. } => OrigPose::Pivot(def.pivot),
                    },
                });
            } else {
                self.selected = None;
                self.edit_pid = None;
            }
        }

        if down {
            match (&self.drag, self.selected) {
                (Some(Drag::Node { grab, orig }), Some(sel)) => {
                    let delta_world = world - *grab;
                    if delta_world.length() > 0.0001 {
                        let path = ancestors(root, sel);
                        let center_local = match orig {
                            OrigPose::MeshRect(r) => vec2(r.x + r.w / 2.0, r.y + r.h / 2.0),
                            OrigPose::WarpRect(r) => vec2(r.x + r.w / 2.0, r.y + r.h / 2.0),
                            OrigPose::Pivot(p) => *p,
                        };
                        let delta = world_delta_to_local(&path, params, center_local, delta_world);
                        let node = node_at_mut(root, sel).unwrap();
                        match (node, orig) {
                            (Node::Mesh(m), OrigPose::MeshRect(r)) => {
                                m.rect.x = r.x + delta.x;
                                m.rect.y = r.y + delta.y;
                            }
                            (Node::Warp { def, .. }, OrigPose::WarpRect(r)) => {
                                def.rect.x = r.x + delta.x;
                                def.rect.y = r.y + delta.y;
                            }
                            (Node::Rot { def, .. }, OrigPose::Pivot(p)) => {
                                def.pivot = *p + delta;
                            }
                            _ => {}
                        }
                    }
                }
                (Some(Drag::GridPoint { warp_id, idx }), _) => {
                    let wid = *warp_id;
                    let idx = *idx;
                    let Some(wf) = ev.warps.iter().find(|w| w.node_id == wid) else { return };
                    let cur_world = wf.pts[idx];
                    let delta_world = world - cur_world;
                    // immutable gather: path + local position + grid size
                    let (div_x, div_y, local) = {
                        let path = ancestors(root, wid);
                        let node: &Node = match node_at(root, wid) {
                            Some(n) => n,
                            None => return,
                        };
                        let Node::Warp { def, .. } = node else { return };
                        let nx = def.div_x + 1;
                        let i = idx % nx;
                        let j = idx / nx;
                        let (u, v) = (i as f32 / def.div_x as f32, j as f32 / def.div_y as f32);
                        let mut p = vec2(def.rect.x + u * def.rect.w, def.rect.y + v * def.rect.h);
                        for d in &def.drivers {
                            let (dx, dy, deg) = d.apply(params.norm(d.pid), u, v, def.rect);
                            if deg != 0.0 {
                                if let DriverKind::RotBy { pivot, .. } = d.kind {
                                    p = apply_rot(p, pivot, deg);
                                }
                            }
                            p.x += dx;
                            p.y += dy;
                        }
                        (def.div_x, def.div_y, p)
                    };
                    let path = ancestors(root, wid);
                    let delta_local = world_delta_to_local(&path, params, local, delta_world);
                    let n_grid = (div_x + 1) * (div_y + 1);
                    // mutable edit
                    let n = match node_at_mut(root, wid) {
                        Some(n) => n,
                        None => return,
                    };
                    let Node::Warp { def, .. } = n else { return };
                    if def.base_offsets.len() < n_grid {
                        def.base_offsets.resize(n_grid, Vec2::ZERO);
                    }
                    if let Some(pid) = self.edit_pid {
                        let nrm = params.norm(pid);
                        let kf = match def.keyforms.iter_mut().find(|k| k.pid == pid) {
                            Some(k) => k,
                            None => {
                                def.keyforms.push(Keyform { pid, pos: vec![Vec2::ZERO; n_grid], neg: vec![Vec2::ZERO; n_grid] });
                                def.keyforms.last_mut().unwrap()
                            }
                        };
                        if nrm > 0.05 {
                            kf.pos[idx] += delta_local / nrm;
                        } else if nrm < -0.05 {
                            kf.neg[idx] += delta_local / (-nrm);
                        } else {
                            def.base_offsets[idx] += delta_local;
                        }
                    } else {
                        def.base_offsets[idx] += delta_local;
                    }
                }
                _ => {}
            }
        }
    }

    // ---------- panels ----------

    pub fn draw_tool_details(&mut self, ui: &ui::Ui, area: Rect, params: &mut Params) {
        let x = area.x;
        let mut y = area.y + 6.0;
        let lvl = self.edit_level;
        ui.text(&format!("Edit Level {} — {}", lvl, match lvl { 1 => "view & play (parameters only)", 2 => "select & move deformers/meshes", _ => "warp grid + keyform editing" }), x + 6.0, y + 12.0, 11.0, ui::TEXT, true);
        y += 22.0;
        // edit param selector for keyform editing
        ui.text("Keyform target:", x + 6.0, y + 12.0, 10.5, ui::DIM, false);
        let label = match self.edit_pid {
            Some(pid) => format!("{}", params.defs[pid].name),
            None => "shape (no param)".into(),
        };
        if ui::button(ui, x + 100.0, y, 150.0, 20.0, &label, self.edit_pid.is_some(), ui::MINT) {
            // cycle: none -> next param
            self.edit_pid = match self.edit_pid {
                None => Some(0),
                Some(p) => {
                    let next = p + 1;
                    if next >= params.defs.len() { None } else { Some(next) }
                }
            };
            // lock manual control of the edited param so controllers don't fight the editor
            if let Some(pid) = self.edit_pid {
                params.manual.insert(pid, params.values[pid]);
            }
        }
        y += 26.0;
        let hint = if self.edit_level >= 3 {
            "Drag grid points to sculpt. With a keyform target set, edits are recorded at the current parameter value (±1 keyforms), exactly like Cubism."
        } else {
            "Switch to Edit Level 3 to sculpt warp grids."
        };
        ui.text(hint, x + 6.0, y + 12.0, 10.0, ui::DIM, false);
        y += 34.0;
        // add parameter
        ui.text("Add parameter:", x + 6.0, y + 12.0, 10.5, ui::DIM, false);
        if ui::button(ui, x + 100.0, y, 120.0, 20.0, "New parameter…", false, ui::CORAL) {
            self.param_add = Some(ParamAdd { name: "ParamCustom".into(), group_idx: 7, min: -1.0, def: 0.0, max: 1.0 });
        }
        y += 26.0;
        if self.record.is_some() {
            ui.text("● REC", x + 6.0, y + 12.0, 11.0, ui::CORAL, true);
        }
    }

    pub fn draw_param_add(&mut self, ui: &ui::Ui, params: &mut Params) {
        let Some(pa) = &self.param_add else { return };
        let w = 320.0;
        let h = 190.0;
        let x = screen_width() / 2.0 - w / 2.0;
        let y = screen_height() / 2.0 - h / 2.0;
        ui::rounded(x, y, w, h, 10.0, ui::PANEL_HI);
        ui::rounded(x, y, w, 34.0, 10.0, ui::PANEL);
        ui.text("New parameter", x + 14.0, y + 22.0, 13.0, ui::TEXT, true);
        // text input
        ui.text("ID / name", x + 14.0, y + 62.0, 10.5, ui::DIM, false);
        ui::rounded(x + 110.0, y + 48.0, 190.0, 20.0, 6.0, ui::PANEL);
        ui.text(&pa.name, x + 118.0, y + 62.0, 11.0, ui::TEXT, false);
        // steppers
        let rows = [("min", pa.min), ("default", pa.def), ("max", pa.max)];
        let mut vals = [pa.min, pa.def, pa.max];
        for (i, (label, _)) in rows.iter().enumerate() {
            let ry = y + 78.0 + 26.0 * i as f32;
            ui.text(label, x + 14.0, ry + 14.0, 10.5, ui::DIM, false);
            if ui::button(ui, x + 110.0, ry, 24.0, 18.0, "-", false, ui::PANEL_HI) {
                vals[i] -= 0.1;
            }
            ui.text(&format!("{:.2}", vals[i]), x + 158.0, ry + 14.0, 11.0, ui::TEXT, false);
            if ui::button(ui, x + 276.0, ry, 24.0, 18.0, "+", false, ui::PANEL_HI) {
                vals[i] += 0.1;
            }
        }
        // capture chars (owned copy to avoid borrow issues)
        let mut name = pa.name.clone();
        while let Some(c) = get_char_pressed() {
            if c.is_alphanumeric() || c == '_' {
                name.push(c);
            }
        }
        if is_key_down(KeyCode::Backspace) {
            name.pop();
        }
        if let Some(pa) = self.param_add.as_mut() {
            pa.name = name;
            pa.min = vals[0];
            pa.def = vals[1];
            pa.max = vals[2];
        }
        if ui::button(ui, x + 110.0, y + h - 32.0, 130.0, 22.0, "Create", true, ui::MINT) {
            if let Some(pa) = self.param_add.take() {
                let id = pa.name.clone();
                let pid = params.add_param(id.clone(), id, "Custom".into(), pa.min, pa.def, pa.max);
                self.edit_pid = Some(pid);
                params.manual.insert(pid, pa.def);
                self.toast("Parameter created — drag it in the palette, sculpt its keyforms");
            }
        }
        if ui::button(ui, x + w - 90.0, y + h - 32.0, 80.0, 22.0, "Cancel", false, ui::PANEL_HI) {
            self.param_add = None;
        }
    }

    pub fn draw_inspector(&mut self, ui: &ui::Ui, area: Rect, root: &mut Node, params: &Params, ev: &EvalResult) {
        let x = area.x + 6.0;
        let mut y = area.y + 6.0;
        let Some(sel) = self.selected else {
            ui.text("Nothing selected.", x, y + 12.0, 10.5, ui::DIM, false);
            ui.text("Edit Level 2/3: click objects on the canvas or the rig list.", x, y + 30.0, 10.0, ui::DIM, false);
            return;
        };
        let Some(node) = node_at_mut(root, sel) else {
            ui.text("Selection is stale — click again.", x, y + 12.0, 10.5, ui::DIM, false);
            return;
        };
        match node {
            Node::Mesh(m) => {
                ui.text(&format!("Art mesh «{}»", m.part), x, y + 12.0, 11.5, ui::MINT, true);
                y += 22.0;
                ui.text(&format!("rect {:.2},{:.2}  {:.2}x{:.2}   grid {}x{}   opacity {:.2}", m.rect.x, m.rect.y, m.rect.w, m.rect.h, m.div_x, m.div_y, m.opacity(params)), x, y + 12.0, 10.0, ui::TEXT, false);
                y += 20.0;
                y = self.draw_driver_add(ui, x, y, &mut m.drivers, &mut m.opac_drivers, params, false);
            }
            Node::Warp { def, children } => {
                ui.text("Warp deformer", x, y + 12.0, 11.5, ui::MINT, true);
                y += 22.0;
                ui.text(&format!("rect {:.2},{:.2}  {:.2}x{:.2}   grid {}x{}   children {}", def.rect.x, def.rect.y, def.rect.w, def.rect.h, def.div_x, def.div_y, children.len()), x, y + 12.0, 10.0, ui::TEXT, false);
                y += 20.0;
                let kf_info: Vec<String> = def.keyforms.iter().map(|k| format!("{} ({:+.2})", params.defs[k.pid].name, params.norm(k.pid))).collect();
                ui.text(&format!("keyforms: {}  base edits: {}", if kf_info.is_empty() { "none".into() } else { kf_info.join(", ") }, if def.base_offsets.iter().any(|o| o.length_squared() > 1e-8) { "yes" } else { "no" }), x, y + 12.0, 10.0, ui::DIM, false);
                y += 20.0;
                let mut drivers = std::mem::take(&mut def.drivers);
                y = self.draw_driver_add(ui, x, y, &mut drivers, &mut Vec::new(), params, true);
                let node2 = node_at_mut(root, sel).unwrap();
                if let Node::Warp { def, .. } = node2 {
                    def.drivers = drivers;
                }
            }
            Node::Rot { def, .. } => {
                ui.text("Rotation deformer", x, y + 12.0, 11.5, ui::CORAL, true);
                y += 22.0;
                ui.text(&format!("pivot {:.2},{:.2}   angle {:+.1}°", def.pivot.x, def.pivot.y, def.angle(params)), x, y + 12.0, 10.0, ui::TEXT, false);
                y += 20.0;
                let mut drivers = std::mem::take(&mut def.drivers);
                y = self.draw_driver_add(ui, x, y, &mut drivers, &mut Vec::new(), params, true);
                let node2 = node_at_mut(root, sel).unwrap();
                if let Node::Rot { def, .. } = node2 {
                    def.drivers = drivers;
                }
            }
        }
        let _ = ev;
    }

    fn draw_driver_add(&mut self, ui: &ui::Ui, x: f32, y: f32, drivers: &mut Vec<Driver>, opac: &mut Vec<(usize, f32)>, params: &Params, _rot: bool) -> f32 {
        let mut y = y;
        ui.text(&format!("drivers: {}", drivers.len()), x, y + 12.0, 10.0, ui::DIM, false);
        y += 18.0;
        // list with delete buttons
        let mut i = 0;
        while i < drivers.len() {
            let d = &drivers[i];
            let kind = match d.kind {
                DriverKind::ShiftX { .. } => "shift X",
                DriverKind::ShiftY { .. } => "shift Y",
                DriverKind::ScaleX { .. } => "scale X",
                DriverKind::ScaleY { .. } => "scale Y",
                DriverKind::RotBy { .. } => "rotate",
            };
            ui.text(&format!("{} «{}»", kind, params.defs[d.pid].name), x + 8.0, y + 12.0, 9.5, ui::TEXT, false);
            if ui::button(ui, x + 150.0, y, 40.0, 16.0, "del", false, ui::CORAL_DK) {
                drivers.remove(i);
                continue;
            }
            y += 19.0;
            i += 1;
        }
        let mut i = 0;
        while i < opac.len() {
            ui.text(&format!("opacity «{}» ×{:.2}", params.defs[opac[i].0].name, opac[i].1), x + 8.0, y + 12.0, 9.5, ui::TEXT, false);
            if ui::button(ui, x + 150.0, y, 40.0, 16.0, "del", false, ui::CORAL_DK) {
                opac.remove(i);
                continue;
            }
            y += 19.0;
            i += 1;
        }
        // add driver row
        let kinds = ["shift X", "shift Y", "scale X", "scale Y", "rotate"];
        if ui::button(ui, x, y, 62.0, 18.0, kinds[self.driver_kind], false, ui::PANEL_HI) {
            self.driver_kind = (self.driver_kind + 1) % kinds.len();
        }
        let pid = self.driver_param.min(params.defs.len() - 1);
        if ui::button(ui, x + 68.0, y, 110.0, 18.0, &format!("«{}»", params.defs[pid].name), false, ui::PANEL_HI) {
            self.driver_param = (self.driver_param + 1) % params.defs.len();
        }
        if ui::button(ui, x + 184.0, y, 44.0, 18.0, "add", true, ui::MINT) {
            let w = match self.driver_weight {
                0 => Weight::One,
                1 => Weight::VGrad(0.35, 1.0),
                2 => Weight::EdgeX(0.5),
                _ => Weight::Bottom(1.0),
            };
            let kind = match self.driver_kind {
                0 => DriverKind::ShiftX { scale: self.driver_scale, w },
                1 => DriverKind::ShiftY { scale: self.driver_scale, w },
                2 => DriverKind::ScaleX { cu: 0.5, k: self.driver_scale * 2.0, w },
                3 => DriverKind::ScaleY { cv: 0.5, k: self.driver_scale * 2.0, w },
                _ => DriverKind::RotBy { pivot: vec2(1.5, 1.0), deg: self.driver_scale * 200.0, w },
            };
            drivers.push(Driver { pid, kind });
        }
        y += 24.0;
        // weight preset cycle + scale stepper
        let wname = ["uniform", "tips", "edges", "bottom"][self.driver_weight];
        if ui::button(ui, x, y, 84.0, 18.0, &format!("w: {}", wname), false, ui::PANEL_HI) {
            self.driver_weight = (self.driver_weight + 1) % 4;
        }
        ui.text(&format!("scale {:+.3}", self.driver_scale), x + 92.0, y + 13.0, 9.5, ui::DIM, false);
        if ui::button(ui, x + 170.0, y, 20.0, 18.0, "-", false, ui::PANEL_HI) {
            self.driver_scale -= 0.01;
        }
        if ui::button(ui, x + 194.0, y, 20.0, 18.0, "+", false, ui::PANEL_HI) {
            self.driver_scale += 0.01;
        }
        y + 24.0
    }

    pub fn draw_tree_list(&mut self, ui: &ui::Ui, area: Rect, title: &str, filter_kind: Option<&str>) {
        ui.text(title, area.x + 8.0, area.y + 14.0, 11.0, ui::MINT, true);
        let row_h = 18.0;
        let m = mouse_position();
        let mut hover = None;
        let mut click = None;
        // scroll wheel over the panel
        if m.0 >= area.x && m.0 <= area.x + area.w && m.1 >= area.y && m.1 <= area.y + area.h {
            let wh = mouse_wheel().1;
            if wh != 0.0 {
                let idx = if filter_kind == Some("M") { 0 } else { 1 };
                self.tree_scroll[idx] -= wh * 28.0;
            }
        }
        let scroll = &mut self.tree_scroll[if filter_kind == Some("M") { 0 } else { 1 }];
        // clamp
        let visible = self.tree.iter().filter(|n| filter_kind.map(|fk| n.kind == fk).unwrap_or(true)).count() as f32 * row_h;
        *scroll = scroll.min(0.0).max((area.h - 26.0 - visible).min(0.0));
        let mut y = area.y + 24.0 - *scroll;
        let click_row = |y: f32, area: Rect, m: (f32, f32), row_h: f32| -> bool {
            m.0 >= area.x && m.0 <= area.x + area.w && m.1 >= y && m.1 <= y + row_h && y >= area.y && y + row_h <= area.y + area.h
        };
        for n in self.tree.clone() {
            if let Some(fk) = filter_kind {
                if n.kind != fk {
                    continue;
                }
            }
            if y >= area.y - row_h && y + row_h <= area.y + area.h + 2.0 {
                let inside = click_row(y, area, m, row_h);
                if inside {
                    hover = Some(n.id);
                    if is_mouse_button_pressed(MouseButton::Left) {
                        click = Some(n.id);
                    }
                }
                let x = area.x + 8.0 + n.depth as f32 * 12.0;
                if self.selected == Some(n.id) {
                    ui::rounded(area.x + 4.0, y, area.w - 8.0, row_h, 4.0, ui::PANEL_HI);
                } else if inside {
                    ui::rounded(area.x + 4.0, y, area.w - 8.0, row_h, 4.0, ui::PANEL_HI);
                }
                let (col, tag) = match n.kind {
                    "W" => (ui::MINT, "W"),
                    "R" => (ui::CORAL, "R"),
                    _ => (ui::DIM, "M"),
                };
                ui::rounded(x, y + 3.0, 16.0, 13.0, 3.0, Color::new(col.r, col.g, col.b, 0.2));
                ui.text(tag, x + 5.5, y + 13.0, 8.5, col, true);
                ui.text(&n.label, x + 22.0, y + 13.0, 9.5, if inside || self.selected == Some(n.id) { ui::TEXT } else { ui::DIM }, false);
            }
            y += row_h;
        }
        if let Some(id) = click {
            self.selected = Some(id);
            self.edit_pid = None;
        }
        self.hover_node = hover.or(self.hover_node);
    }

    pub fn draw_overlays_editor(&mut self, ui: &ui::Ui, ev: &EvalResult, ppu: f32, ox: f32, oy: f32) {
        // reuse generic overlays, then paint the selection
        ui::draw_overlays(ui, ev, ppu, ox, oy, self.hover_node, self.show_grid, self.show_mesh);
        if let Some(sel) = self.selected {
            let to_s = |p: Vec2| vec2(ox + p.x * ppu, oy + p.y * ppu);
            // grid points of selected warp
            if let Some(wf) = ev.warps.iter().find(|w| w.node_id == sel) {
                if self.edit_level >= 3 {
                    for (i, p) in wf.pts.iter().enumerate() {
                        let s = to_s(*p);
                        let dragging = matches!(&self.drag, Some(Drag::GridPoint { idx, .. }) if *idx == i);
                        draw_circle(s.x, s.y, if dragging { 6.0 } else { 4.0 }, if dragging { ui::CORAL } else { ui::TEXT });
                    }
                }
            }
            if let Some(b) = ev.mesh_bbox(sel) {
                let s = vec2(ox + b.x * ppu, oy + b.y * ppu);
                let e = vec2(ox + (b.x + b.w) * ppu, oy + (b.y + b.h) * ppu);
                draw_rectangle_lines(s.x - 3.0, s.y - 3.0, e.x - s.x + 6.0, e.y - s.y + 6.0, 2.0, ui::CORAL);
            } else if let Some(wb) = ev.warp_bbox(sel) {
                let s = vec2(ox + wb.x * ppu, oy + wb.y * ppu);
                let e = vec2(ox + (wb.x + wb.w) * ppu, oy + (wb.y + wb.h) * ppu);
                draw_rectangle_lines(s.x - 3.0, s.y - 3.0, e.x - s.x + 6.0, e.y - s.y + 6.0, 2.0, ui::MINT);
                let _ = wb;
            }
            if let Some(r) = ev.rots.iter().find(|r| r.node_id == sel) {
                let s = to_s(r.pivot);
                draw_circle_lines(s.x, s.y, 0.09 * ppu, 2.0, ui::CORAL);
                draw_line(s.x - 8.0, s.y, s.x + 8.0, s.y, 2.0, ui::CORAL);
                draw_line(s.x, s.y - 8.0, s.x, s.y + 8.0, 2.0, ui::CORAL);
            }
        }
    }

    pub fn draw_toast(&self, ui: &ui::Ui) {
        if let Some((msg, until)) = &self.toast {
            if get_time() < *until {
                let w = ui.text_w(msg, 11.5, false) + 26.0;
                let x = screen_width() / 2.0 - w / 2.0;
                ui::rounded(x, 116.0, w, 26.0, 8.0, ui::PANEL_HI);
                ui.text(msg, x + 13.0, 133.0, 11.5, ui::TEXT, false);
            }
        }
    }

    pub fn draw_about(&mut self, ui: &ui::Ui) {
        if !self.about {
            return;
        }
        let w = 460.0;
        let h = 240.0;
        let x = screen_width() / 2.0 - w / 2.0;
        let y = screen_height() / 2.0 - h / 2.0;
        ui::rounded(x, y, w, h, 10.0, ui::PANEL_HI);
        ui.text("CubiCraft Editor 1.0", x + 20.0, y + 34.0, 16.0, ui::TEXT, true);
        let lines = [
            "A clean-room, Cubism-style modeling + runtime in pure Rust.",
            "",
            "Study -> Build -> Check -> Ready (craft-anything method).",
            "No Live2D code, formats or assets are used. Models are saved",
            "in the open .ccm3 / .cca3 JSON formats documented in the README.",
            "",
            "github.com/eren-coder07/cubicraft — MIT license.",
        ];
        let mut yy = y + 64.0;
        for l in lines {
            ui.text(l, x + 20.0, yy, 11.0, ui::DIM, false);
            yy += 20.0;
        }
        if ui::button(ui, x + w - 90.0, y + h - 36.0, 70.0, 24.0, "Close", false, ui::PANEL_HI) {
            self.about = false;
        }
    }
}

/// Hit-testing: prefer small meshes, then warps, then rotation pivots.
pub fn pick_node(ev: &EvalResult, world: Vec2, root: &Node, params: &Params) -> Option<u32> {
    let mut best: Option<(u32, f32)> = None;
    for m in ev.meshes.iter().rev() {
        let b = match ev.mesh_bbox(m.node_id) {
            Some(b) => b,
            None => continue,
        };
        if world.x >= b.x && world.x <= b.x + b.w && world.y >= b.y && world.y <= b.y + b.h {
            let area = (b.w * b.h).max(0.0001);
            if best.map(|(_, ba)| area < ba).unwrap_or(true) {
                best = Some((m.node_id, area));
            }
        }
    }
    if best.is_none() {
        for w in ev.warps.iter().rev() {
            let b = points_bbox(&w.pts);
            if world.x >= b.x && world.x <= b.x + b.w && world.y >= b.y && world.y <= b.y + b.h {
                let area = (b.w * b.h).max(0.0001);
                if best.map(|(_, ba)| area < ba).unwrap_or(true) {
                    best = Some((w.node_id, area));
                }
            }
        }
    }
    if best.is_none() {
        for r in ev.rots.iter().rev() {
            if world.distance(r.pivot) < 0.06 {
                return Some(r.node_id);
            }
        }
    }
    let _ = (root, params);
    best.map(|(id, _)| id)
}
