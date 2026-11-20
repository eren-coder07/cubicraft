// CubiCraft — UI.
// A Cubism-Editor-inspired parameter palette, rig tree, guide and overlays,
// drawn with macroquad primitives. Dark studio theme.

use macroquad::prelude::*;

use crate::deform::{EvalResult, TreeNodeInfo};
use crate::params::{Params, GROUPS};

// theme
pub const BG_TOP: Color = Color::new(0.086, 0.070, 0.114, 1.0);
pub const BG_BOT: Color = Color::new(0.122, 0.098, 0.153, 1.0);
pub const PANEL: Color = Color::new(0.125, 0.102, 0.165, 1.0);
pub const PANEL_HI: Color = Color::new(0.165, 0.137, 0.216, 1.0);
pub const BORDER: Color = Color::new(0.196, 0.165, 0.243, 1.0);
pub const TEXT: Color = Color::new(0.937, 0.914, 0.961, 1.0);
pub const DIM: Color = Color::new(0.655, 0.624, 0.722, 1.0);
pub const MINT: Color = Color::new(0.557, 0.847, 0.753, 1.0);
pub const CORAL: Color = Color::new(1.0, 0.561, 0.463, 1.0);
pub const CORAL_DK: Color = Color::new(0.878, 0.420, 0.333, 1.0);
pub const GLOW: Color = Color::new(0.557, 0.847, 0.753, 0.10);

pub struct Fonts {
    pub bold: Font,
    pub reg: Font,
}

pub struct Ui {
    pub fonts: Fonts,
    pub tab: usize, // 0 params, 1 tree, 2 guide
    pub scroll: [f32; 2],
    pub tree: Vec<TreeNodeInfo>,
    pub hover_node: Option<u32>,
    pub toggle_grid: bool,
    pub toggle_mesh: bool,
    pub physics_on: bool,
    pub blink_on: bool,
    pub look_on: bool,
    pub expr: usize,
    pub menu_hover_x: f32,
    pub menu_open_idx: Option<usize>,
}

pub const TABS: [&str; 3] = ["PARAMS", "RIG", "GUIDE"];

impl Ui {
    pub fn new() -> Self {
        let bold_bytes = include_bytes!("../assets/fonts/Poppins-SemiBold.ttf");
        let reg_bytes = include_bytes!("../assets/fonts/Poppins-Regular.ttf");
        let bold = load_ttf_font_from_bytes(bold_bytes).expect("bold font");
        let reg = load_ttf_font_from_bytes(reg_bytes).expect("reg font");
        Ui {
            fonts: Fonts { bold, reg },
            tab: 0,
            scroll: [0.0; 2],
            tree: vec![],
            hover_node: None,
            toggle_grid: false,
            toggle_mesh: false,
            physics_on: true,
            blink_on: true,
            look_on: true,
            expr: 0,
            menu_hover_x: 0.0,
            menu_open_idx: None,
        }
    }

    pub fn text(&self, s: &str, x: f32, y: f32, size: f32, color: Color, bold: bool) {
        let f = if bold { &self.fonts.bold } else { &self.fonts.reg };
        draw_text_ex(s, x, y, TextParams { font: Some(f), font_size: size as u16, color, ..Default::default() });
    }

    pub fn text_w(&self, s: &str, size: f32, bold: bool) -> f32 {
        let f = if bold { &self.fonts.bold } else { &self.fonts.reg };
        measure_text(s, Some(f), size as u16, 1.0).width
    }
}

pub fn panel_bg(area: Rect) {
    draw_rectangle(area.x, area.y, area.w, area.h, PANEL);
    draw_rectangle(area.x + area.w - 2.0, area.y, 2.0, area.h, BORDER);
    draw_rectangle(area.x, area.y + area.h - 2.0, area.w, 2.0, BORDER);
}

pub fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    draw_rectangle(x, y, w, h, color);
    if r > 0.0 {
        draw_circle(x + r, y + r, r, color);
        draw_circle(x + w - r, y + r, r, color);
        draw_circle(x + r, y + h - r, r, color);
        draw_circle(x + w - r, y + h - r, r, color);
        draw_rectangle(x, y + r, w, h - 2.0 * r, color);
        draw_rectangle(x + r, y, w - 2.0 * r, h, color);
    }
}

pub fn button(ui: &Ui, x: f32, y: f32, w: f32, h: f32, label: &str, active: bool, accent: Color) -> bool {
    let m = mouse_position();
    let hot = m.0 >= x && m.0 <= x + w && m.1 >= y && m.1 <= y + h;
    let base = if active { accent } else if hot { PANEL_HI } else { PANEL };
    rounded(x, y, w, h, 8.0, base);
    let tcol = if active { Color::new(0.05, 0.04, 0.07, 1.0) } else if hot { TEXT } else { DIM };
    let tw = ui.text_w(label, 12.0, true);
    ui.text(label, x + w / 2.0 - tw / 2.0, y + h / 2.0 + 4.5, 12.0, tcol, true);
    hot && is_mouse_button_pressed(MouseButton::Left)
}

pub struct SliderOut {
    pub active: bool,
    pub released: bool,
    pub value: f32,
}

pub fn slider_v(ui: &Ui, x: f32, y: f32, w: f32, min: f32, max: f32, val: f32, accent: Color, ro: bool) -> SliderOut {
    let bar_x = x + 118.0;
    let bar_w = w - 118.0 - 44.0;
    let bar_y = y + 2.0;
    let bar_h = 10.0;
    rounded(bar_x, bar_y, bar_w, bar_h, 5.0, Color::new(0.0, 0.0, 0.0, 0.35));
    let t = ((val - min) / (max - min)).clamp(0.0, 1.0);
    let fw = (bar_w - 4.0) * t;
    let col = if ro { DIM } else { accent };
    rounded(bar_x + 2.0, bar_y + 2.0, fw, bar_h - 4.0, 4.0, col);
    if !ro {
        rounded(bar_x + 2.0 + fw - 3.0, bar_y - 1.0, 6.0, bar_h + 2.0, 3.0, TEXT);
    }
    let m = mouse_position();
    let in_bar = m.0 >= bar_x && m.0 <= bar_x + bar_w && m.1 >= bar_y - 8.0 && m.1 <= bar_y + bar_h + 8.0;
    let down = is_mouse_button_down(MouseButton::Left);
    let pressed = is_mouse_button_pressed(MouseButton::Left);
    let released = is_mouse_button_released(MouseButton::Left);
    let mut out = SliderOut { active: false, released: false, value: val };
    if !ro && in_bar && (pressed || down) {
        let t2 = ((m.0 - bar_x) / bar_w).clamp(0.0, 1.0);
        out.value = min + (max - min) * t2;
        out.active = true;
        out.released = released;
    }
    out
}

pub fn draw_background(area: Rect, time: f64) {
    let steps = 24;
    for i in 0..steps {
        let t = i as f32 / steps as f32;
        let c = Color::new(BG_TOP.r + (BG_BOT.r - BG_TOP.r) * t, BG_TOP.g + (BG_BOT.g - BG_TOP.g) * t, BG_TOP.b + (BG_BOT.b - BG_TOP.b) * t, 1.0);
        draw_rectangle(area.x, area.y + t * area.h, area.w, area.h / steps as f32 + 1.0, c);
    }
    let cx = area.x + area.w * 0.5;
    let cy = area.y + area.h * 0.48;
    for i in 0..5 {
        let r = area.h * (0.18 + 0.07 * i as f32);
        let a = (0.05 - 0.008 * i as f32).max(0.008);
        draw_circle(cx, cy, r, Color::new(MINT.r, MINT.g, MINT.b, a));
    }
    let t = time as f32;
    for i in 0..14 {
        let seed = i as f32 * 7.13;
        let sx = area.x + area.w * (0.5 + 0.42 * (seed * 1.7).sin());
        let sy = area.y + area.h * (0.5 + 0.44 * (seed * 2.3).cos());
        let tw = 0.5 + 0.5 * (t * (0.6 + 0.25 * (seed % 1.0)) + seed).sin();
        let r = 1.6 + 1.4 * tw;
        let col = if i % 3 == 0 { CORAL } else { MINT };
        draw_circle(sx, sy, r * tw, Color::new(col.r, col.g, col.b, 0.35 * tw));
    }
}

pub fn draw_ground_shadow(ppu: f32, ox: f32, oy: f32) {
    let cx = ox + 1.5 * ppu;
    let cy = oy + 2.68 * ppu;
    let rx = 0.55 * ppu;
    draw_ellipse(cx, cy, rx, rx * 0.14, 0.0, Color::new(0.0, 0.0, 0.0, 0.30));
}

pub fn draw_overlays(ui: &Ui, ev: &EvalResult, ppu: f32, ox: f32, oy: f32, hover: Option<u32>, grid_on: bool, mesh_on: bool) {
    let to_s = |p: &Vec2| vec2(ox + p.x * ppu, oy + p.y * ppu);
    if grid_on {
        for w in &ev.warps {
            let (nx, ny) = (w.div_x + 1, w.div_y + 1);
            let hot = hover == Some(w.node_id);
            let col = if hot { CORAL } else { MINT };
            let a = if hot { 0.95 } else { 0.40 };
            for j in 0..ny {
                for i in 0..w.div_x {
                    let a0 = to_s(&w.pts[j * nx + i]);
                    let a1 = to_s(&w.pts[j * nx + i + 1]);
                    draw_line(a0.x, a0.y, a1.x, a1.y, if hot { 2.0 } else { 1.0 }, Color::new(col.r, col.g, col.b, a));
                }
            }
            for i in 0..nx {
                for j in 0..w.div_y {
                    let a0 = to_s(&w.pts[j * nx + i]);
                    let a1 = to_s(&w.pts[(j + 1) * nx + i]);
                    draw_line(a0.x, a0.y, a1.x, a1.y, if hot { 2.0 } else { 1.0 }, Color::new(col.r, col.g, col.b, a));
                }
            }
            for q in &w.pts {
                let s = to_s(q);
                draw_circle(s.x, s.y, 2.5, Color::new(CORAL.r, CORAL.g, CORAL.b, a));
            }
        }
        for r in &ev.rots {
            let s = to_s(&r.pivot);
            let col = if hover == Some(r.node_id) { CORAL } else { CORAL_DK };
            draw_circle_lines(s.x, s.y, 0.07 * ppu, 2.0, Color::new(col.r, col.g, col.b, 0.8));
            draw_line(s.x - 6.0, s.y, s.x + 6.0, s.y, 1.5, col);
            draw_line(s.x, s.y - 6.0, s.x, s.y + 6.0, 1.5, col);
        }
    }
    if mesh_on {
        for m in &ev.meshes {
            let hot = hover == Some(m.node_id);
            let col = Color::new(1.0, 1.0, 1.0, if hot { 0.8 } else { 0.20 });
            for t in m.idx.chunks(3) {
                let a = to_s(&m.verts[t[0] as usize]);
                let b = to_s(&m.verts[t[1] as usize]);
                let c = to_s(&m.verts[t[2] as usize]);
                draw_line(a.x, a.y, b.x, b.y, 1.0, col);
                draw_line(b.x, b.y, c.x, c.y, 1.0, col);
                draw_line(c.x, c.y, a.x, a.y, 1.0, col);
            }
            if hot {
                for v in &m.verts {
                    let s = to_s(v);
                    draw_circle(s.x, s.y, 2.6, CORAL);
                }
            }
        }
    }
    if let Some(h) = hover {
        if let Some(b) = ev.mesh_bbox(h) {
            let s = vec2(ox + b.x * ppu, oy + b.y * ppu);
            let e = vec2(ox + (b.x + b.w) * ppu, oy + (b.y + b.h) * ppu);
            draw_rectangle(s.x - 4.0, s.y - 4.0, e.x - s.x + 8.0, e.y - s.y + 8.0, GLOW);
            draw_rectangle_lines(s.x - 4.0, s.y - 4.0, e.x - s.x + 8.0, e.y - s.y + 8.0, 2.0, CORAL);
        }
    }
}

#[allow(dead_code)]
pub fn draw_topbar(ui: &Ui, w: f32, fps: f32) -> Rect {
    draw_rectangle(0.0, 0.0, w, 46.0, PANEL);
    draw_rectangle(0.0, 44.0, w, 2.0, BORDER);
    ui.text("CubiCraft", 18.0, 30.0, 21.0, TEXT, true);
    let sub = "a clean-room Live2D-style engine · pure Rust + macroquad";
    ui.text(sub, 150.0, 29.0, 11.5, DIM, false);
    let chips = [("STUDY", MINT), ("BUILD", MINT), ("CHECK", MINT), ("READY", CORAL)];
    let mut cx = w - chips.len() as f32 * 74.0 - 110.0;
    for (name, col) in chips {
        rounded(cx, 13.0, 62.0, 20.0, 10.0, Color::new(col.r, col.g, col.b, 0.16));
        let tw = ui.text_w(name, 10.0, true);
        ui.text(name, cx + 31.0 - tw / 2.0, 27.0, 10.0, col, true);
        cx += 74.0;
    }
    let fps_s = format!("{:.0} fps", fps);
    ui.text(&fps_s, w - 62.0, 27.0, 11.5, DIM, false);
    Rect::new(0.0, 0.0, w, 46.0)
}

#[allow(dead_code)]
pub fn draw_canvas_hints(ui: &Ui, area: Rect) {
    let hints = "move mouse: track · click body/head: motions · tune rig in PARAMS panel";
    let tw = ui.text_w(hints, 11.0, false);
    ui.text(hints, area.x + area.w / 2.0 - tw / 2.0, area.y + area.h - 14.0, 11.0, Color::new(DIM.r, DIM.g, DIM.b, 0.8), false);
    let sig = "warp deformers · pendulum physics · motion clips · expressions · eye-blink · breath";
    let tw2 = ui.text_w(sig, 9.0, false);
    ui.text(sig, area.x + area.w / 2.0 - tw2 / 2.0, area.y + area.h - 2.0, 9.0, Color::new(DIM.r, DIM.g, DIM.b, 0.45), false);
}

pub struct BottomBarOut {
    pub wave: bool,
    pub hop: bool,
    pub reset: bool,
    pub phys: bool,
    pub look: bool,
    pub blink: bool,
    pub expr: Option<usize>,
}

#[allow(dead_code)]
pub fn draw_bottombar(ui: &mut Ui, area: Rect, playing_motion: Option<&str>) -> BottomBarOut {
    let h = 56.0;
    let y = area.y + area.h - h;
    let w = area.w;
    draw_rectangle(area.x, y, w, h + 2.0, PANEL);
    draw_rectangle(area.x, y, w, 2.0, BORDER);
    let mut x = area.x + 14.0;
    ui.text("MOTION", x, y + 24.0, 10.0, DIM, true);
    x += 62.0;
    let wave = button(ui, x, y + 12.0, 74.0, 30.0, "Wave", playing_motion == Some("Wave"), MINT);
    x += 80.0;
    let hop = button(ui, x, y + 12.0, 74.0, 30.0, "Hop", playing_motion == Some("Hop"), MINT);
    x += 88.0;
    ui.text("EMOTE", x, y + 24.0, 10.0, DIM, true);
    x += 56.0;
    let exprs = ["N", "Smile", "Angry", "Wow"];
    let mut expr = None;
    for (i, e) in exprs.iter().enumerate() {
        let bw = if i == 0 { 32.0 } else { 60.0 };
        if button(ui, x, y + 12.0, bw, 30.0, e, ui.expr == i, CORAL) {
            expr = Some(i);
        }
        x += bw + 8.0;
    }
    // toggles, right-aligned
    let bx = area.x + w - 14.0;
    let mut x = bx - 66.0;
    let reset = button(ui, x, y + 12.0, 66.0, 30.0, "Reset", false, CORAL_DK);
    x -= 84.0;
    let blink = button(ui, x, y + 12.0, 78.0, 30.0, if ui.blink_on { "Blink on" } else { "Blink off" }, ui.blink_on, if ui.blink_on { MINT } else { DIM });
    x -= 86.0;
    let look = button(ui, x, y + 12.0, 80.0, 30.0, if ui.look_on { "Look on" } else { "Look off" }, ui.look_on, if ui.look_on { MINT } else { DIM });
    x -= 88.0;
    let phys = button(ui, x, y + 12.0, 82.0, 30.0, if ui.physics_on { "Phys on" } else { "Phys off" }, ui.physics_on, if ui.physics_on { MINT } else { DIM });
    BottomBarOut { wave, hop, reset, phys, look, blink, expr }
}

#[allow(dead_code)]
pub fn draw_panel_frame(ui: &Ui, w: f32, h: f32) -> Rect {
    let px = w - 318.0;
    draw_rectangle(px, 46.0, 318.0, h - 46.0, PANEL);
    draw_rectangle(px, 46.0, 2.0, h - 46.0, BORDER);
    let mut tx = px + 10.0;
    for (i, t) in TABS.iter().enumerate() {
        let active = ui.tab == i;
        let tw = ui.text_w(t, 12.0, true);
        if active {
            rounded(tx - 6.0, 56.0, tw + 12.0, 24.0, 7.0, PANEL_HI);
            draw_rectangle(tx - 6.0, 76.0, tw + 12.0, 2.5, MINT);
        }
        ui.text(t, tx, 72.0, 12.0, if active { TEXT } else { DIM }, true);
        tx += tw + 24.0;
    }
    Rect::new(px, 46.0, 318.0, h - 46.0)
}

pub fn panel_content_area(panel: Rect) -> Rect {
    Rect::new(panel.x + 12.0, panel.y + 44.0, panel.w - 24.0, panel.h - 56.0)
}

pub fn clamp_scroll(scroll: f32, content_h: f32, view_h: f32) -> f32 {
    scroll.min(0.0).max((view_h - content_h).min(0.0))
}

pub fn param_rows(ui: &mut Ui, params: &mut Params, area: Rect) {
    let mut scroll = ui.scroll[0];
    let row_h = 26.0;
    let mut y = area.y - scroll;
    let x = area.x;
    let w = area.w;
    let mut content_h = 0.0;
    for g in GROUPS {
        rounded(x, y, w, 22.0, 6.0, PANEL_HI);
        ui.text(g, x + 8.0, y + 15.0, 11.0, MINT, true);
        y += 28.0;
        content_h += 28.0;
        for pid in 0..params.defs.len() {
            if params.defs[pid].group != *g {
                continue;
            }
            let d = &params.defs[pid];
            let manual = params.manual.contains_key(&pid);
            let name = if manual { format!("{} *", d.name) } else { d.name.to_string() };
            ui.text(&name, x, y + 11.0, 10.5, TEXT, true);
            let out = slider_v(ui, x, y, w, d.min, d.max, params.values[pid], if d.physics { CORAL } else { MINT }, false);
            if out.active {
                let v = out.value.clamp(d.min, d.max);
                params.manual.insert(pid, v);
                params.values[pid] = v;
            }
            let vs = format!("{:.2}", params.values[pid]);
            ui.text(&vs, x + w - 34.0, y + 11.0, 10.0, DIM, false);
            if d.physics {
                ui.text("phy", x + w - 40.0 - ui.text_w("phy", 8.0, true) - 8.0, y + 11.0, 8.0, CORAL, true);
            }
            y += row_h;
            content_h += row_h;
        }
        y += 4.0;
    }
    let auto = button(ui, x, y + 2.0, w, 30.0, "AUTO — release all sliders", params.manual.is_empty(), CORAL);
    if auto {
        params.manual.clear();
    }
    content_h += 40.0;
    let m = mouse_position();
    if m.0 >= area.x && m.0 <= area.x + area.w && m.1 >= area.y && m.1 <= area.y + area.h {
        let wh = mouse_wheel().1;
        if wh != 0.0 {
            scroll -= wh * 40.0;
        }
    }
    ui.scroll[0] = clamp_scroll(scroll, content_h, area.h);
}

#[allow(dead_code)]
pub fn tree_rows(ui: &mut Ui, area: Rect) -> Option<u32> {
    let mut scroll = ui.scroll[1];
    let row_h = 24.0;
    let mut y = area.y - scroll;
    let mut content_h = 0.0;
    let mut hover = None;
    let m = mouse_position();
    for n in &ui.tree {
        if y + row_h > area.y && y < area.y + area.h {
            let x = area.x + n.depth as f32 * 16.0;
            let inside = m.0 >= area.x && m.0 <= area.x + area.w && m.1 >= y && m.1 <= y + row_h;
            if inside {
                hover = Some(n.id);
                draw_rectangle(area.x, y, area.w, row_h, PANEL_HI);
            }
            let (col, label) = match n.kind {
                "W" => (MINT, "WARP"),
                "R" => (CORAL, "ROT"),
                _ => (DIM, "MESH"),
            };
            rounded(x, y + 5.0, 34.0, 14.0, 4.0, Color::new(col.r, col.g, col.b, 0.18));
            let tw = ui.text_w(label, 8.0, true);
            ui.text(label, x + 17.0 - tw / 2.0, y + 15.5, 8.0, col, true);
            let desc = match n.kind {
                "W" => "warp deformer (grid)",
                "R" => "rotation deformer",
                _ => "art mesh (textured tris)",
            };
            ui.text(desc, x + 42.0, y + 15.5, 10.5, if inside { TEXT } else { DIM }, false);
        }
        y += row_h;
        content_h += row_h;
    }
    let note_y = area.y - scroll + content_h + 12.0;
    ui.text("hover a node to highlight it on the canvas", area.x, note_y, 10.0, DIM, false);
    content_h += 26.0;
    if m.0 >= area.x && m.0 <= area.x + area.w && m.1 >= area.y && m.1 <= area.y + area.h {
        let wh = mouse_wheel().1;
        if wh != 0.0 {
            scroll -= wh * 40.0;
        }
    }
    ui.scroll[1] = clamp_scroll(scroll, content_h, area.h);
    hover
}

#[allow(dead_code)]
pub fn guide_rows(ui: &mut Ui, area: Rect) {
    let mut scroll = ui.scroll[1];
    let x = area.x;
    let mut y = area.y - scroll;
    let mut content_h = 0.0;
    {
        ui.text("How Live2D Cubism works — studied", x, y + 14.0, 12.0, MINT, true);
        y += 26.0;
        content_h += 26.0;
        let lines = [
            "A Cubism rig = layered art (PSD parts) + a deformer tree + parameters.",
            "Every frame the runtime writes parameter values (motions, expressions,",
            "eye-blink, breath, look-at, physics), then evaluates deformers and draws",
            "each art mesh as textured triangles in rig order.",
        ];
        for l in lines {
            ui.text(l, x, y + 13.0, 10.0, TEXT, false);
            y += 17.0;
            content_h += 17.0;
        }
        y += 8.0;
        content_h += 8.0;
        let lines = [
            "Warp deformers: NxM control grids, per-cell bilinear mapping.",
            "Rotation deformers: pivot + angle from parameters.",
            "Files: .moc3 (rig), .model3.json, .physics3.json (pendulums),",
            ".motion3.json (curves + fades), .exp3.json (expression sets).",
            "Eyeballs move via clipping masks; motions fade in/out and loop.",
        ];
        for l in lines {
            ui.text(l, x, y + 13.0, 10.0, TEXT, false);
            y += 17.0;
            content_h += 17.0;
        }
    }
    y += 14.0;
    content_h += 14.0;
    ui.text("What CubiCraft rebuilds (clean-room)", x, y + 14.0, 12.0, CORAL, true);
    y += 26.0;
    content_h += 26.0;
    let lines = [
        "• Parameter system — standard Cubism IDs, groups, min/max/def",
        "• Warp + rotation deformers with per-vertex keyform drivers",
        "• Single 2048px atlas baked at startup; textured art-mesh pipeline",
        "• .physics3.json-style spring pendulums driving hair sway (phys)",
        "• .motion3.json-style clips: IdleSway / Wave / Hop + fade weights",
        "• .exp3.json-style expressions, eye-blink, breath, mouse look-at",
        "• Iris parallax via layered meshes (Cubism uses clip masks)",
        "• Clickable rig tree + warp-grid/mesh overlays to see the machinery",
    ];
    for l in lines {
        ui.text(l, x, y + 13.0, 10.0, TEXT, false);
        y += 17.0;
        content_h += 17.0;
    }
    y += 14.0;
    content_h += 14.0;
    ui.text("Study sources", x, y + 14.0, 12.0, DIM, true);
    y += 26.0;
    content_h += 26.0;
    let lines = [
        "Live2D Editor manual (deformers, standard parameter list), Cubism",
        "SDK for Web framework (physics evaluation, motion blending) and",
        "craft-anything's Study -> Build -> Check -> Ready method.",
        "",
        "CubiCraft contains no Live2D code or assets — every part of this rig,",
        "art and engine is original and written from scratch in Rust.",
    ];
    for l in lines {
        ui.text(l, x, y + 13.0, 10.0, DIM, false);
        y += 17.0;
        content_h += 17.0;
    }
    let m = mouse_position();
    if m.0 >= area.x && m.0 <= area.x + area.w && m.1 >= area.y && m.1 <= area.y + area.h {
        let wh = mouse_wheel().1;
        if wh != 0.0 {
            scroll -= wh * 40.0;
        }
    }
    ui.scroll[1] = clamp_scroll(scroll, content_h + 20.0, area.h);
}

#[allow(dead_code)]
pub fn canvas_overlays_buttons(ui: &mut Ui, _area: Rect) {
    let x = 12.0;
    let y = 58.0;
    // custom toggle buttons (drawn over canvas)
    let m = mouse_position();
    let mk = |ui: &Ui, x: f32, w: f32, label: &str, on: bool| {
        let hot = m.0 >= x && m.0 <= x + w && m.1 >= y && m.1 <= y + 26.0;
        let col = if on { MINT } else if hot { PANEL_HI } else { PANEL };
        rounded(x, y, w, 26.0, 8.0, col);
        let tcol = if on { Color::new(0.05, 0.04, 0.07, 1.0) } else { TEXT };
        let tw = ui.text_w(label, 11.0, true);
        ui.text(label, x + w / 2.0 - tw / 2.0, y + 17.0, 11.0, tcol, true);
    };
    mk(ui, x, 70.0, "Grid", ui.toggle_grid);
    mk(ui, x + 76.0, 80.0, "Mesh", ui.toggle_mesh);
    if m.0 >= x && m.0 <= x + 70.0 && m.1 >= y && m.1 <= y + 26.0 && is_mouse_button_pressed(MouseButton::Left) {
        ui.toggle_grid = !ui.toggle_grid;
    }
    let x2 = x + 76.0;
    if m.0 >= x2 && m.0 <= x2 + 80.0 && m.1 >= y && m.1 <= y + 26.0 && is_mouse_button_pressed(MouseButton::Left) {
        ui.toggle_mesh = !ui.toggle_mesh;
    }
}
