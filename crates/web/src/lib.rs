//! Cubicraft web shell — presents the CPU framebuffer to a <canvas> element.

use cubicraft_core::app::App;
use cubicraft_core::render::Fb;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

const RIG_JSON: &str = include_str!("../../../assets/rig.json");

#[wasm_bindgen]
pub struct WebApp {
    app: App,
    fb: Option<Fb>,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
}

#[wasm_bindgen]
impl WebApp {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement, psd: &[u8]) -> Result<WebApp, JsValue> {
        console_error_panic_hook::set_once();
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")?
            .ok_or_else(|| JsValue::from_str("no 2d context"))?
            .dyn_into()?;
        let app = App::new(psd, RIG_JSON).map_err(|e| JsValue::from_str(&e))?;
        Ok(WebApp { app, fb: None, canvas, ctx })
    }

    /// Set the canvas backing-store size (call on resize / DPR change).
    pub fn resize(&mut self, w: u32, h: u32) {
        self.canvas.set_width(w);
        self.canvas.set_height(h);
        self.fb = Some(Fb::new(w, h));
    }

    /// Advance one frame and blit it to the canvas.
    pub fn frame(&mut self, dt_ms: f32) {
        let Some(fb) = self.fb.as_mut() else { return };
        self.app.step((dt_ms / 1000.0).clamp(0.001, 0.05));
        self.app.render(fb);
        let img = match web_sys::ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&fb.buf),
            fb.w,
            fb.h,
        ) {
            Ok(i) => i,
            Err(_) => return,
        };
        let _ = self.ctx.put_image_data(&img, 0.0, 0.0);
    }

    /// Pointer move in canvas backing-store pixels.
    pub fn pointer(&mut self, x: f32, y: f32) {
        self.app.set_pointer(x, y);
    }

    /// Press/release state change (drag anchor is captured on press).
    pub fn press(&mut self, down: bool) {
        self.app.set_down(down);
    }

    pub fn pointer_left(&mut self) {
        self.app.pointer_left();
    }

    /// Keyboard: "Space" blink, "KeyT" talk, "KeyR" reset.
    pub fn key(&mut self, code: &str) {
        match code {
            "Space" => self.app.blink_now(),
            "KeyT" => self.app.toggle_talk(),
            "KeyR" => self.app.pointer_left(),
            _ => {}
        }
    }
}
