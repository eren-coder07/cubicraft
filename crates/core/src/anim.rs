//! Idle animation controller: breathing, blinking, eye wander, look-at,
//! talking. All values are written into the parameter registry.

use crate::math::{clamp, pursue, Vec2};
use crate::rig::Params;

/// Tiny deterministic PRNG (xorshift64) — no external dependencies.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn f32(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }
}

pub struct Anim {
    pub t: f32,
    next_blink: f32,
    blink_t: Option<f32>,
    blink_dur: f32,
    look: Vec2,
    eyeball: Vec2,
    wander: Vec2,
    wander_target: Vec2,
    next_wander: f32,
    rng: Rng,
    pub talking: bool,
}

impl Anim {
    pub fn new() -> Self {
        Self {
            t: 0.0,
            next_blink: 2.2,
            blink_t: None,
            blink_dur: 0.18,
            look: Vec2::default(),
            eyeball: Vec2::default(),
            wander: Vec2::default(),
            wander_target: Vec2::default(),
            next_wander: 1.5,
            rng: Rng(0x9E3779B97F4A7C15),
            talking: false,
        }
    }

    pub fn request_blink(&mut self) {
        self.blink_t = Some(0.0);
    }

    /// `look_target` is the normalized pointer position ([-1,1] range),
    /// `look_active` false when the pointer left the window.
    pub fn update(&mut self, params: &mut Params, dt: f32, look_target: Vec2, look_active: bool) {
        self.t += dt;
        let t = self.t;

        // look pursuit (decays back to center when idle)
        let rate = if look_active { 6.5 } else { 2.2 };
        let target = if look_active { look_target } else { Vec2::default() };
        self.look.x = pursue(self.look.x, target.x, rate, dt);
        self.look.y = pursue(self.look.y, target.y, rate, dt);

        // eye wander
        if t >= self.next_wander {
            self.wander_target = Vec2::new(self.rng.range(-0.55, 0.55), self.rng.range(-0.4, 0.4));
            self.next_wander = t + self.rng.range(1.4, 3.4);
        }
        self.wander.x = pursue(self.wander.x, self.wander_target.x, 3.0, dt);
        self.wander.y = pursue(self.wander.y, self.wander_target.y, 3.0, dt);
        self.eyeball.x = pursue(
            self.eyeball.x,
            clamp(self.look.x * 1.0 + self.wander.x * 0.35, -1.0, 1.0),
            9.0,
            dt,
        );
        self.eyeball.y = pursue(
            self.eyeball.y,
            clamp(self.look.y * 1.0 + self.wander.y * 0.35, -1.0, 1.0),
            9.0,
            dt,
        );

        // blinking
        if self.blink_t.is_none() && t >= self.next_blink {
            self.blink_t = Some(0.0);
            self.next_blink = t + self.rng.range(2.2, 5.6);
        }
        let mut eye_open = 1.0f32;
        if let Some(bt) = &mut self.blink_t {
            *bt += dt;
            let p = clamp(*bt / self.blink_dur, 0.0, 1.0);
            let c = (std::f32::consts::PI * p).sin().powf(1.3);
            eye_open = 1.0 - c;
            if p >= 1.0 {
                self.blink_t = None;
                // 20% chance of an immediate double blink
                if self.rng.f32() < 0.2 {
                    self.blink_t = Some(0.0);
                }
            }
        }
        params.set("EyeLOpen", eye_open);
        params.set("EyeROpen", eye_open);
        params.set("EyeBallX", self.eyeball.x);
        params.set("EyeBallY", self.eyeball.y);

        // talking
        let mouth = if self.talking {
            (t * 7.2).sin().abs() * (0.45 + 0.55 * (t * 2.1).sin().abs())
        } else {
            0.0
        };
        params.set("MouthOpenY", mouth);

        // head: pointer look + layered idle motion
        let idle_x = 4.0 * (t * 0.9).sin() + 2.0 * (t * 2.1).sin();
        let idle_y = 2.0 * (t * 0.7 + 1.3).sin();
        let idle_z = 1.6 * (t * 0.5 + 0.7).sin();
        params.set("AngleX", self.look.x * 20.0 + idle_x);
        params.set("AngleY", self.look.y * 13.0 + idle_y);
        params.set("AngleZ", self.look.x * 2.5 + idle_z);

        // body: breathing + lazy sway
        params.set("BodyAngleY", 1.6 * (t * 1.7).sin());
        params.set("BodyAngleZ", 0.8 * (t * 0.8).sin());
        params.set("BodyAngleX", 1.3 * (t * 0.55 + 1.0).sin());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blinks_periodically() {
        let mut a = Anim::new();
        let mut p = Params::new();
        let mut blinks = 0;
        let mut last_open = 1.0;
        for _ in 0..1800 {
            a.update(&mut p, 1.0 / 60.0, Vec2::default(), false);
            let open = p.get("EyeLOpen");
            if open < 0.5 && last_open >= 0.5 {
                blinks += 1;
            }
            last_open = open;
        }
        // 30 seconds -> expect at least 2 blinks
        assert!(blinks >= 2, "blinks={blinks}");
    }

    #[test]
    fn talk_moves_mouth() {
        let mut a = Anim::new();
        a.talking = true;
        let mut p = Params::new();
        let mut moved = false;
        let mut prev = 0.0;
        for _ in 0..300 {
            a.update(&mut p, 1.0 / 60.0, Vec2::default(), false);
            if (p.get("MouthOpenY") - prev).abs() > 1e-3 {
                moved = true;
            }
            prev = p.get("MouthOpenY");
        }
        assert!(moved);
        assert!(p.get("MouthOpenY") >= 0.0);
    }
}
