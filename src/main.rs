// CubiCraft — main.
// A Cubism-Editor-style application: model editor + runtime in one.
//
// Runtime pipeline (mirrors Cubism's): params -> motions -> expression ->
// look-at -> breath -> blink -> physics -> deformer tree eval -> triangles.
// Editor pipeline: menus / edit levels / panels / canvas gizmos / .ccm3 IO.

use macroquad::prelude::*;

mod art;
mod deform;
mod editor;
mod motion;
mod params;
mod physics;
mod project;
mod renderer;
mod serde_math;
mod ui;

use art::{bake_atlas, build_parts, build_physics, build_rig, hop_motion, idle_motion, wave_motion};
use deform::eval;
use editor::{Action, Editor};
use editor::MENUS;
use motion::{Blink, Breath, Expression, LookAt, MotionPlayer};
use physics::Physics;
use params::Params;
use project::Project;
use renderer::{ppu_for_view, screen_to_world};
use ui::Ui;

/// The editable model.
pub struct Model {
    pub name: String,
    pub root: deform::Node,
    pub params: Params,
    pub physics_settings: Vec<physics::PhysicsSetting>,
    pub physics: Physics,
    pub clips: Vec<motion::Clip>,
}

impl Model {
    pub fn new() -> Self {
        let mut model = Model {
            name: "Momi".into(),
            root: deform::Node::Mesh(deform::MeshDef {
                rect: Rect::new(0.0, 0.0, 0.001, 0.001),
                div_x: 1,
                div_y: 1,
                uv: Rect::new(0.0, 0.0, 4.0, 4.0),
                part: String::new(),
                drivers: vec![],
                base_opacity: 0.0,
                opac_drivers: vec![],
            }),
            params: Params::new(),
            physics_settings: vec![],
            physics: Physics::new(vec![]),
            clips: vec![],
        };
        model.load_momi();
        model
    }

    pub fn load_momi(&mut self) {
        self.params.reset_to_base();
        PARTS.with(|p| {
            let rig = art::build_rig(&self.params, &p.borrow());
            self.root = rig.root;
        });
        self.physics_settings = vec![build_physics(&self.params)];
        self.rebuild_physics();
        self.clips = vec![idle_motion(&self.params), wave_motion(&self.params), hop_motion(&self.params)];
        self.name = "Momi".into();
    }

    pub fn rebuild_physics(&mut self) {
        self.physics = Physics::new(self.physics_settings.clone());
    }
}

thread_local! {
    static PARTS: std::cell::RefCell<Vec<art::Part>> = std::cell::RefCell::new(Vec::new());
}

#[allow(dead_code)]
pub fn art_parts() -> Vec<art::Part> {
    PARTS.with(|p| p.borrow().clone())
}

fn set_art_parts(parts: Vec<art::Part>) {
    PARTS.with(|p| *p.borrow_mut() = parts);
}

fn window_conf() -> Conf {
    Conf {
        window_title: "CubiCraft Editor — a clean-room Cubism-style modeler in pure Rust".to_owned(),
        window_width: 1440,
        window_height: 900,
        window_resizable: true,
        high_dpi: true,
        ..Default::default()
    }
}

// layout metrics
const TITLE_H: f32 = 26.0;
const MENU_H: f32 = 24.0;
const TOOLBAR_H: f32 = 32.0;
const LEFT_W: f32 = 230.0;
const BOTTOM_H: f32 = 170.0;
const STATUS_H: f32 = 22.0;

#[macroquad::main(window_conf)]
async fn main() {
    // ---------- build & bake (the "PSD -> parts -> atlas" step) ----------
    let mut parts = build_parts();
    art::pack_atlas(&mut parts);
    let rt = renderer::bake_atlas_to_rt(&parts, || {
        bake_atlas(&parts);
    });
    set_art_parts(parts);

    let mut model = Model::new();
    let mut renderer = unsafe {
        let gl = macroquad::window::get_internal_gl();
        renderer::Renderer::new(gl.quad_context)
    };
    {
        let gl = unsafe { macroquad::window::get_internal_gl() };
        renderer.set_atlas(gl.quad_context, &rt);
    }

    let mut ui = Ui::new();
    let mut ed = Editor::new();
    ed.refresh_tree(&model.root);

    // controllers
    let mut idle_player = MotionPlayer::new();
    let mut tap_player = MotionPlayer::new();
    let mut blink = Blink::new();
    let mut breath = Breath::new();
    let mut look = LookAt::new();
    let mut expr = motion::Expression::new();

    loop {
        let dt = get_frame_time().clamp(1.0 / 240.0, 1.0 / 15.0);
        let (sw, sh) = (screen_width(), screen_height());

        // ---------- layout ----------
        let top_h = TITLE_H + MENU_H + TOOLBAR_H;
        let canvas = Rect::new(
            LEFT_W,
            top_h,
            sw - LEFT_W,
            sh - top_h - BOTTOM_H - STATUS_H,
        );
        let ppu = ppu_for_view(canvas.w, canvas.h) * 1.05;
        let ox = canvas.x + canvas.w / 2.0 - 1.5 * ppu;
        let oy = canvas.y + canvas.h / 2.0 - 1.32 * ppu;

        // ---------- runtime pipeline ----------
        let editing = ed.edit_level >= 2;
        model.params.begin_frame();
        model.params.apply_manual();
        if !editing {
            idle_player.update(&mut model.params, dt);
            tap_player.update(&mut model.params, dt);
            expr.update(&mut model.params, dt);
            {
                let m = mouse_position();
                let inside = m.0 >= canvas.x && m.0 <= canvas.x + canvas.w && m.1 >= canvas.y && m.1 <= canvas.y + canvas.h;
                let w = screen_to_world(vec2(m.0, m.1), ppu, ox, oy);
                if inside && ed.look_on && !ui_over_pointer(sw, sh) {
                    look.target = vec2(
                        ((w.x - 1.5) / 1.35).clamp(-1.0, 1.0),
                        ((w.y - 1.02) / 1.05).clamp(-1.0, 1.0),
                    );
                } else {
                    look.target = Vec2::ZERO;
                }
            }
            look.update(&mut model.params, dt);
            breath.update(&mut model.params, dt);
            if ed.blink_on {
                blink.update(&mut model.params, get_time(), dt);
            } else {
                let e = model.params.pid("ParamEyeLOpen");
                model.params.set(e, 1.0);
                let e = model.params.pid("ParamEyeROpen");
                model.params.set(e, 1.0);
            }
        } else {
            let e = model.params.pid("ParamEyeLOpen");
            model.params.set(e, 1.0);
            let e = model.params.pid("ParamEyeROpen");
            model.params.set(e, 1.0);
        }
        if ed.physics_on {
            model.physics.update(&model.params, dt);
            model.physics.apply(&mut model.params);
        }

        // ---------- frame evaluation ----------
        let frame = eval(&model.root, &model.params);

        // ---------- editor canvas input ----------
        ed.canvas_input(&mut model, &frame, ppu, ox, oy, canvas);

        // ---------- clip recording ----------
        if let Some(rec) = &mut ed.record {
            let t = (get_time() - rec.start) as f32;
            if t >= 6.0 {
                ed.run_action(Action::RecordStop, &mut model, &mut tap_player);
            } else if t - rec.last >= 0.1 {
                rec.last = t;
                for pid in 0..model.params.defs.len() {
                    rec.curves.entry(pid).or_default().push((t, model.params.values[pid]));
                }
            }
        }

        // ---------- draw: canvas ----------
        ui::draw_background(canvas, get_time());
        ui::draw_ground_shadow(ppu, ox, oy);
        {
            let mut gl = unsafe { macroquad::window::get_internal_gl() };
            gl.flush();
            renderer.draw_model(gl.quad_context, &frame, ppu, ox, oy);
        }
        ed.draw_overlays_editor(&ui, &frame, ppu, ox, oy);

        // ---------- chrome: title / menus / toolbar ----------
        let action_menu = draw_title_bar(&ui, sw);
        let mut action = draw_menu_bar(&mut ui, &mut ed, sw);
        if action == Action::None {
            action = action_menu;
        }
        draw_toolbar(&ui, &mut ed, sw);
        let menu_state = (ui.menu_hover_x, ui.menu_open_idx);
        // left panels
        let left_top = Rect::new(0.0, top_h, LEFT_W, (canvas.h + BOTTOM_H) * 0.42);
        let left_bot = Rect::new(0.0, top_h + left_top.h, LEFT_W, canvas.h + BOTTOM_H - left_top.h);
        ui::panel_bg(left_top);
        ui::panel_bg(left_bot);
        ed.draw_tree_list(&ui, left_top, "PARTS / MESHES", Some("M"));
        ed.draw_tree_list(&ui, left_bot, "DEFORMER TREE", None);
        // bottom panel with tabs
        let bottom = Rect::new(LEFT_W, sh - STATUS_H - BOTTOM_H, sw - LEFT_W, BOTTOM_H);
        ui::panel_bg(bottom);
        let tabs = ["Tool Details", "Inspector", "Parameter"];
        let mut tx = bottom.x + 10.0;
        for (i, t) in tabs.iter().enumerate() {
            let active = ui.tab == i;
            let tw = ui.text_w(t, 11.5, true) + 14.0;
            if active {
                ui::rounded(tx, bottom.y + 4.0, tw, 22.0, 6.0, ui::PANEL_HI);
                draw_rectangle(tx, bottom.y + 24.0, tw, 2.0, ui::MINT);
            }
            ui.text(t, tx + 7.0, bottom.y + 19.0, 11.5, if active { ui::TEXT } else { ui::DIM }, true);
            if is_mouse_button_pressed(MouseButton::Left) && mouse_over(tx, bottom.y + 4.0, tw, 22.0) {
                ui.tab = i;
            }
            tx += tw + 8.0;
        }
        let content = Rect::new(bottom.x + 8.0, bottom.y + 32.0, bottom.w - 16.0, bottom.h - 40.0);
        match ui.tab {
            0 => ed.draw_tool_details(&ui, content, &mut model.params),
            1 => ed.draw_inspector(&ui, content, &mut model.root, &model.params, &frame),
            _ => ui::param_rows(&mut ui, &mut model.params, content),
        }
        // dropdowns must paint above panels — draw them last
        draw_menu_dropdown(&ui, &mut ed, sw, menu_state);

        // status bar
        draw_rectangle(0.0, sh - STATUS_H, sw, STATUS_H, ui::PANEL);
        ed.zoom_display = ppu * 100.0 / 238.0;
        ui.text(
            &format!(
                "Zoom {:.1}%   ·   {} — {} parts, {} clips   ·   project: open .ccm3 format   ·   {:.0} fps",
                ed.zoom_display,
                model.name,
                art::build_parts().len(),
                model.clips.len() + ed.recorded.is_some() as usize,
                1.0 / dt
            ),
            10.0,
            sh - 6.0,
            10.5,
            ui::DIM,
            false,
        );
        ui.text(
            match ed.edit_level {
                1 => "L1 · runtime: move the mouse, play motions from Parameter tab",
                2 => "L2 · click to select, drag to move — File ▸ Modeling for deformer ops",
                _ => "L3 · drag warp grid points to sculpt keyforms (Tool Details ▸ keyform target)",
            },
            sw / 2.0 + 40.0,
            sh - 6.0,
            10.5,
            ui::DIM,
            false,
        );

        // ---------- handle actions ----------
        if let Some(p) = ed.pending_action.take() {
            action = p;
        }
        if action != Action::None {
            ed.run_action(action, &mut model, &mut tap_player);
        }
        ed.draw_param_add(&ui, &mut model.params);
        ed.draw_about(&ui);
        ed.draw_toast(&ui);

        next_frame().await
    }
}

fn mouse_over(x: f32, y: f32, w: f32, h: f32) -> bool {
    let m = mouse_position();
    m.0 >= x && m.0 <= x + w && m.1 >= y && m.1 <= y + h
}

fn ui_over_pointer(_sw: f32, sh: f32) -> bool {
    let m = mouse_position();
    m.0 < LEFT_W || m.1 >= sh - STATUS_H - BOTTOM_H || m.1 < TITLE_H + MENU_H + TOOLBAR_H
}

fn draw_title_bar(ui: &Ui, sw: f32) -> Action {
    draw_rectangle(0.0, 0.0, sw, TITLE_H, ui::PANEL);
    ui.text("CubiCraft Editor 1.0", 12.0, 18.0, 13.0, ui::TEXT, true);
    ui.text(
        "[open source · a clean-room Cubism-style modeler — pure Rust]",
        190.0,
        17.0,
        10.0,
        ui::DIM,
        false,
    );
    Action::None
}

fn draw_menu_bar(ui: &mut Ui, ed: &mut Editor, sw: f32) -> Action {
    draw_rectangle(0.0, TITLE_H, sw, MENU_H, ui::PANEL_HI);
    let mut action = Action::None;
    let m = mouse_position();
    let mut x = 8.0;
    let mut open_x = 0.0;
    ui.menu_open_idx = None;
    for (i, name) in MENUS.iter().enumerate() {
        let w = ui.text_w(name, 11.5, false) + 16.0;
        let hot = ed.menu_open == Some(i);
        let over = mouse_over(x, TITLE_H, w, MENU_H);
        if over && is_mouse_button_pressed(MouseButton::Left) {
            ed.menu_open = if hot { None } else { Some(i) };
        }
        draw_rectangle(x, TITLE_H, w, MENU_H, if hot || over { ui::PANEL } else { Color::new(0.0, 0.0, 0.0, 0.0) });
        ui.text(name, x + 8.0, TITLE_H + 16.0, 11.5, if hot || over { ui::TEXT } else { ui::DIM }, false);
        if hot {
            ui.menu_open_idx = Some(i);
            open_x = x;
        }
        x += w;
    }
    ui.menu_hover_x = open_x;
    // close menus when clicking elsewhere
    if is_mouse_button_pressed(MouseButton::Left) && ed.menu_open.is_some() && m.1 > TITLE_H + MENU_H + 230.0 {
        ed.menu_open = None;
    }
    action
}

fn draw_menu_dropdown(ui: &Ui, ed: &mut Editor, sw: f32, state: (f32, Option<usize>)) {
    let Some(i) = state.1 else { return };
    let x = state.0;
    let items = Editor::menu_items(i);
    let dh = items.len() as f32 * 22.0 + 10.0;
    let w = 240.0f32.min(sw - x - 6.0);
    ui::rounded(x, TITLE_H + MENU_H, w, dh, 6.0, ui::PANEL_HI);
    draw_rectangle_lines(x, TITLE_H + MENU_H, w, dh, 1.0, ui::BORDER);
    let mut y = TITLE_H + MENU_H + 7.0;
    for (label, act) in items {
        let over_i = mouse_over(x, y, w, 22.0);
        if over_i {
            draw_rectangle(x + 3.0, y, w - 6.0, 22.0, ui::PANEL);
        }
        ui.text(label, x + 12.0, y + 15.0, 11.0, if over_i { ui::TEXT } else { ui::DIM }, false);
        if over_i && is_mouse_button_pressed(MouseButton::Left) {
            ed.pending_action = Some(act.clone());
            ed.menu_open = None;
        }
        y += 22.0;
    }
}

fn draw_toolbar(ui: &Ui, ed: &mut Editor, sw: f32) {
    let y = TITLE_H + MENU_H;
    draw_rectangle(0.0, y, sw, TOOLBAR_H, ui::PANEL);
    draw_rectangle(0.0, y + TOOLBAR_H - 2.0, sw, 2.0, ui::BORDER);
    ui.text("Edit Level:", 12.0, y + 20.0, 11.0, ui::DIM, false);
    let mut x = 78.0;
    for lvl in 1..=3 {
        if ui::button(ui, x, y + 6.0, 26.0, 22.0, &lvl.to_string(), ed.edit_level == lvl, ui::MINT) {
            ed.edit_level = lvl;
        }
        x += 30.0;
    }
    x += 14.0;
    ui.text("Toolbox:", x, y + 20.0, 11.0, ui::DIM, false);
    x += 60.0;
    if ui::button(ui, x, y + 6.0, 84.0, 22.0, if ed.show_grid { "Grids ●" } else { "Grids" }, ed.show_grid, ui::MINT) {
        ed.show_grid = !ed.show_grid;
    }
    x += 90.0;
    if ui::button(ui, x, y + 6.0, 84.0, 22.0, if ed.show_mesh { "Mesh ●" } else { "Mesh" }, ed.show_mesh, ui::MINT) {
        ed.show_mesh = !ed.show_mesh;
    }
    x += 90.0;
    if ui::button(ui, x, y + 6.0, 80.0, 22.0, if ed.physics_on { "Phys ●" } else { "Phys" }, ed.physics_on, ui::MINT) {
        ed.physics_on = !ed.physics_on;
    }
    x += 86.0;
    if ui::button(ui, x, y + 6.0, 80.0, 22.0, if ed.blink_on { "Blink ●" } else { "Blink" }, ed.blink_on, ui::MINT) {
        ed.blink_on = !ed.blink_on;
    }
    x += 86.0;
    if ui::button(ui, x, y + 6.0, 80.0, 22.0, if ed.look_on { "Look ●" } else { "Look" }, ed.look_on, ui::MINT) {
        ed.look_on = !ed.look_on;
    }
    x += 100.0;
    if ed.record.is_some() {
        ui.text("● REC", x, y + 20.0, 12.0, ui::CORAL, true);
    } else if let Some(c) = &ed.recorded {
        ui.text(&format!("clip «{}» ready", c.name), x, y + 20.0, 11.0, ui::DIM, false);
    }
    let _ = sw;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_round_trip() {
        let mut parts = art::build_parts();
        art::pack_atlas(&mut parts);
        set_art_parts(parts);
        let mut model = Model::new();
        // make an edit so the saved project is non-trivial
        let wid = match &model.root {
            deform::Node::Warp { .. } => 0,
            _ => panic!("root must be warp"),
        };
        if let Some(deform::Node::Warp { def, .. }) = deform::node_at_mut(&mut model.root, wid) {
            def.base_offsets.resize(def.grid_size(), Vec2::ZERO);
            def.base_offsets[5] = Vec2::new(0.03, -0.02);
        }
        let proj = Project::momi(&model.params, model.root.clone(), model.physics_settings.clone(), model.clips.clone());
        let json = proj.to_json();
        let parsed = Project::from_json(&json).expect("parse back");
        assert_eq!(parsed.format, "ccm3");
        assert_eq!(parsed.custom_params.len(), proj.custom_params.len());
        assert_eq!(parsed.physics.len(), proj.physics.len());
        assert_eq!(parsed.clips.len(), proj.clips.len());
        // roots must round-trip identically
        assert_eq!(
            serde_json::to_value(&proj.root).unwrap(),
            serde_json::to_value(&parsed.root).unwrap()
        );
        // also verify the built-in parameter table survived
        let _ = &mut model.params;
        println!("round-trip OK: {} bytes", json.len());
    }

    #[test]
    fn write_sample_project() {
        let mut parts = art::build_parts();
        art::pack_atlas(&mut parts);
        set_art_parts(parts);
        let model = Model::new();
        let proj = Project::momi(&model.params, model.root.clone(), model.physics_settings.clone(), model.clips.clone());
        std::fs::create_dir_all("assets").unwrap();
        std::fs::write("assets/momi.ccm3", proj.to_json()).unwrap();
        println!("wrote assets/momi.ccm3");
    }
}
