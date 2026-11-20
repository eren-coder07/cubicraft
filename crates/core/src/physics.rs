//! Spring-damper pendulum simulation for hair sway (clean-room re-creation of
//! the pendulum physics model documented in Live2D's physics format).

#[derive(Debug, Clone)]
pub struct Pendulum {
    pub angle: f32,
    pub vel: f32,
    pub stiffness: f32,
    pub damping: f32,
}

impl Pendulum {
    pub fn new(stiffness: f32, damping: f32) -> Self {
        Self { angle: 0.0, vel: 0.0, stiffness, damping }
    }

    /// Advance the simulation. `target` is the source angle (radians), e.g. the
    /// head's horizontal swing; `input_vel` is the source angular velocity,
    /// which injects energy when the head moves fast.
    pub fn update(&mut self, target: f32, input_vel: f32, dt: f32) {
        let acc = (target - self.angle) * self.stiffness - self.vel * self.damping + input_vel;
        // clamp dt for stability on hiccups
        let dt = dt.min(1.0 / 20.0);
        self.vel += acc * dt;
        self.angle += self.vel * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settles_toward_target() {
        let mut p = Pendulum::new(46.0, 7.0);
        for _ in 0..600 {
            p.update(0.4, 0.0, 1.0 / 60.0);
        }
        assert!((p.angle - 0.4).abs() < 0.02);
        assert!(p.vel.abs() < 0.05);
    }

    #[test]
    fn stable_with_lag() {
        let mut p = Pendulum::new(46.0, 7.0);
        let dt = 1.0 / 60.0;
        for t in 0..1200 {
            let src = (t as f32 * dt * 3.0).sin() * 0.3;
            p.update(src, 0.0, dt);
            assert!(p.angle.is_finite());
            assert!(p.angle.abs() < 4.0, "diverged: {}", p.angle);
        }
    }
}
