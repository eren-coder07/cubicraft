//! Application: input state, animation loop, physics binding, rendering.

use crate::anim::Anim;
use crate::math::{clamp, pursue, Vec2};
use crate::model::Model;
use crate::physics::Pendulum;
use crate::render::Fb;
use crate::rig::{parse_spec, PhysSpec};

pub struct App {
    pub model: Model,
    pub anim: Anim,
    phys: Pendulum,
    phys2: Pendulum,
    phys_spec: PhysSpec,
    vw: f32,
    vh: f32,
    pointer: Vec2,
    pointer_active: bool,
    down: bool,
    drag_anchor: Vec2,
    drag_val: Vec2,
    dragging: bool,
    prev_src: f32,
    /// Pose mode: disables idle animation, params are set directly (screenshots).
    pub pose_mode: bool,
}

impl App {
    pub fn new(psd_bytes: &[u8], rig_json: &str) -> Result<App, String> {
        let psd = crate::psd::Psd::parse(psd_bytes)?;
        let spec = parse_spec(rig_json)?;
        let phys_spec = spec.physics.clone();
        let model = Model::build(&psd, &spec)?;
        Ok(App {
            phys: Pendulum::new(phys_spec.stiffness, phys_spec.damping),
            phys2: Pendulum::new(phys_spec.stiffness * 1.5, phys_spec.damping * 1.1),
            phys_spec,
            model,
            anim: Anim::new(),
            vw: 0.0,
            vh: 0.0,
            pointer: Vec2::default(),
            pointer_active: false,
            down: false,
            drag_anchor: Vec2::default(),
            drag_val: Vec2::default(),
            dragging: false,
            prev_src: 0.0,
            pose_mode: false,
        })
    }

    pub fn set_viewport(&mut self, w: u32, h: u32) {
        self.vw = w as f32;
        self.vh = h as f32;
    }

    pub fn set_pointer(&mut self, x: f32, y: f32) {
        self.pointer = Vec2::new(x, y);
        self.pointer_active = true;
    }

    pub fn pointer_left(&mut self) {
        self.pointer_active = false;
        self.down = false;
        self.dragging = false;
    }

    pub fn set_down(&mut self, down: bool) {
        self.down = down;
        if down {
            self.drag_anchor = self.pointer;
            self.dragging = true;
        } else {
            self.dragging = false;
        }
    }

    pub fn toggle_talk(&mut self) {
        self.anim.talking = !self.anim.talking;
    }

    pub fn blink_now(&mut self) {
        self.anim.request_blink();
    }

    fn scale(&self) -> f32 {
        let mw = self.model.canvas.x.max(1.0);
        let mh = self.model.canvas.y.max(1.0);
        ((self.vw / mw).min(self.vh / mh)) * 0.97
    }

    fn look_target(&self) -> Vec2 {
        if self.vw <= 0.0 || self.vh <= 0.0 {
            return Vec2::default();
        }
        Vec2::new(
            clamp((self.pointer.x - self.vw * 0.5) / (self.vw * 0.45), -1.0, 1.0),
            clamp((self.pointer.y - self.vh * 0.40) / (self.vh * 0.45), -1.0, 1.0),
        )
    }

    /// Advance animation + physics by `dt` seconds and evaluate deformers.
    pub fn step(&mut self, dt: f32) {
        let dt = dt.max(1.0 / 240.0).min(0.1);
        let s = self.scale().max(0.0001);

        // drag -> DragX/DragY in canvas pixels (spring back on release)
        if self.dragging {
            let raw = Vec2::new(
                (self.pointer.x - self.drag_anchor.x) / s,
                (self.pointer.y - self.drag_anchor.y) / s,
            );
            let target = Vec2::new(clamp(raw.x, -90.0, 90.0), clamp(raw.y, -40.0, 40.0));
            self.drag_val.x = pursue(self.drag_val.x, target.x, 14.0, dt);
            self.drag_val.y = pursue(self.drag_val.y, target.y, 14.0, dt);
        } else {
            self.drag_val.x = pursue(self.drag_val.x, 0.0, 7.0, dt);
            self.drag_val.y = pursue(self.drag_val.y, 0.0, 7.0, dt);
        }
        self.model.params.set("DragX", self.drag_val.x);
        self.model.params.set("DragY", self.drag_val.y);

        if !self.pose_mode {
            let look = self.look_target();
            let active = self.pointer_active;
            self.anim.update(&mut self.model.params, dt, look, active);
        }

        // physics: pendulums chase the head swing + drag velocity
        let src = self.model.params.get("AngleX").to_radians() * 0.5
            + self.model.params.get("DragX") * 0.004;
        let src_vel = (src - self.prev_src) / dt;
        self.phys.update(src, src_vel * 0.10, dt);
        self.phys2.update(src * 1.25, src_vel * 0.16, dt);
        self.prev_src = src;
        self.model
            .params
            .set("HairSway", self.phys.angle * self.phys_spec.amp);
        self.model
            .params
            .set("HairSway2", self.phys2.angle * self.phys_spec.amp2);

        self.model.eval();
    }

    /// Render the current frame into the framebuffer.
    pub fn render(&mut self, fb: &mut Fb) {
        self.set_viewport(fb.w, fb.h);
        fb.clear(17, 18, 27);
        let s = self.scale();
        let ox = (fb.w as f32 - self.model.canvas.x * s) * 0.5;
        let oy = (fb.h as f32 - self.model.canvas.y * s) * 0.5;

        // floor shadow
        let shadow_cx = (self.model.canvas.x * 0.5) * s + ox + self.model.params.get("DragX") * s;
        let shadow_cy = 2470.0 * s + oy;
        fb.fill_ellipse(shadow_cx, shadow_cy, 210.0 * s, 40.0 * s, 0, 0, 0, 0.45);

        let n = self.model.part_count();
        for i in 0..n {
            let (part, frame) = {
                let p = self.model.part(i);
                let f = &self.model.frames()[i];
                (p, f)
            };
            fb.draw_part(part, frame, s, ox, oy);
        }
    }

    /// Render a named static pose (for screenshots / README art).
    pub fn render_pose(&mut self, name: &str, fb: &mut Fb) {
        self.pose_mode = true;
        self.anim.talking = false;
        self.model.params.reset();
        self.drag_val = Vec2::default();
        self.model.params.set("DragX", 0.0);
        match name {
            "idle" => {}
            "look" => {
                self.model.params.set("AngleX", -22.0);
                self.model.params.set("AngleY", 10.0);
                self.model.params.set("EyeBallX", -0.8);
                self.model.params.set("EyeBallY", 0.35);
                self.model.params.set("HairSway", -7.0);
                self.model.params.set("BodyAngleZ", -1.5);
            }
            "blink" => {
                self.model.params.set("EyeLOpen", 0.04);
                self.model.params.set("EyeROpen", 0.04);
                self.model.params.set("AngleY", -6.0);
            }
            "happy" => {
                self.model.params.set("MouthOpenY", 0.5);
                self.model.params.set("EyeLOpen", 0.0);
                self.model.params.set("EyeROpen", 0.0);
                self.model.params.set("AngleZ", 6.0);
                self.model.params.set("HairSway", 6.0);
                self.model.params.set("BodyAngleZ", 2.0);
            }
            "talk" => {
                self.model.params.set("MouthOpenY", 0.85);
                self.model.params.set("AngleX", 8.0);
                self.model.params.set("EyeBallX", 0.7);
                self.model.params.set("EyeBallY", -0.2);
            }
            "drag" => {
                self.drag_val = Vec2::new(46.0, 6.0);
                self.model.params.set("DragX", 46.0);
                self.model.params.set("DragY", 6.0);
                self.model.params.set("AngleX", 14.0);
                self.model.params.set("HairSway", 16.0);
                self.model.params.set("HairSway2", 8.0);
                self.model.params.set("BodyAngleZ", 2.5);
            }
            _ => {}
        }
        // settle physics + animation
        for _ in 0..50 {
            self.step(1.0 / 60.0);
        }
        self.render(fb);
    }
}

pub const POSES: &[&str] = &["idle", "look", "blink", "happy", "talk", "drag"];
