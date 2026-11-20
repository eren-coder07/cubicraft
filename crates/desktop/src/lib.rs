//! Cubicraft desktop shell (Windows / Linux / macOS / Android).
//! winit for the window + softbuffer to present the CPU-rendered framebuffer.
//!
//! Modes:
//!   (no args)                       interactive window
//!   --screenshots DIR               render the 6 README poses as PNGs
//!   --frames DIR N                  render N idle frames for GIF assembly
//!   --check                         load model, print part stats, exit

use cubicraft_core::app::{App, POSES};
use cubicraft_core::png::write_png_file;
use cubicraft_core::render::Fb;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

pub const RIG_JSON: &str = include_str!("../../../assets/rig.json");
pub const PSD_BYTES: &[u8] = include_bytes!("../../../assets/model/Set1.psd");

pub fn load_app() -> Result<App, String> {
    App::new(PSD_BYTES, RIG_JSON)
}

pub fn run_screenshots(dir: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut app = load_app()?;
    for pose in POSES {
        let mut fb = Fb::new(810, 1080);
        app.render_pose(pose, &mut fb);
        let path = format!("{dir}/pose_{pose}.png");
        write_png_file(std::path::Path::new(&path), fb.w, fb.h, &fb.buf)
            .map_err(|e| e.to_string())?;
        println!("wrote {path}");
    }
    Ok(())
}

pub fn run_frames(dir: &str, n: usize) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut app = load_app()?;
    let mut fb = Fb::new(360, 480);
    for i in 0..n {
        app.step(1.0 / 12.0);
        app.render(&mut fb);
        let path = format!("{dir}/frame_{i:04}.png");
        write_png_file(std::path::Path::new(&path), fb.w, fb.h, &fb.buf)
            .map_err(|e| e.to_string())?;
    }
    println!("wrote {n} frames to {dir}");
    Ok(())
}

pub fn run_check() -> Result<(), String> {
    let app = load_app()?;
    println!("model parts: {}", app.model.part_count());
    std::fs::create_dir_all("/tmp/parts").map_err(|e| e.to_string())?;
    for i in 0..app.model.part_count() {
        let p = app.model.part(i);
        println!(
            "  {:<20} kind={:?} tex={}x{} origin=({:.0},{:.0})",
            p.name,
            p.kind,
            p.tex.w,
            p.tex.h,
            p.origin.x,
            p.origin.y
        );
        if p.tex.w > 1 {
            let path = format!("/tmp/parts/{}.png", p.name);
            write_png_file(std::path::Path::new(&path), p.tex.w, p.tex.h, &p.tex.data)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Interactive window
// ---------------------------------------------------------------------------

struct WinState {
    app: App,
    fb: Option<Fb>,
    window: std::sync::Arc<winit::window::Window>,
    surface: Option<softbuffer::Surface<std::sync::Arc<winit::window::Window>, std::sync::Arc<winit::window::Window>>>,
    size: (u32, u32),
}

impl WinState {
    fn present(&mut self) {
        let (w, h) = self.size;
        if w == 0 || h == 0 {
            return;
        }
        let fb = match &mut self.fb {
            Some(f) if f.w == w && f.h == h => f,
            _ => {
                self.fb = Some(Fb::new(w, h));
                self.fb.as_mut().unwrap()
            }
        };
        self.app.step(1.0 / 60.0);
        self.app.render(fb);
        if let Some(surface) = self.surface.as_mut() {
            let _ = surface.resize(w.try_into().unwrap(), h.try_into().unwrap());
            if let Ok(mut buffer) = surface.buffer_mut() {
                let n = (w as usize * h as usize).min(buffer.len());
                for i in 0..n {
                    let px = &fb.buf[i * 4..i * 4 + 4];
                    buffer[i] =
                        0xFF00_0000 | ((px[0] as u32) << 16) | ((px[1] as u32) << 8) | px[2] as u32;
                }
                let _ = buffer.present();
            }
        }
    }
}

#[derive(Default)]
struct Handler {
    state: Option<WinState>,
}

impl winit::application::ApplicationHandler for Handler {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        let attrs = winit::window::Window::default_attributes()
            .with_title("Cubicraft — clean-room Live2D in pure Rust")
            .with_inner_size(winit::dpi::LogicalSize::new(480.0f32, 900.0))
            .with_min_inner_size(winit::dpi::LogicalSize::new(240.0f32, 450.0));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("window: {e}");
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        let app = match load_app() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("model: {e}");
                event_loop.exit();
                return;
            }
        };
        let window = std::sync::Arc::new(window);
        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("softbuffer ctx: {e}");
                event_loop.exit();
                return;
            }
        };
        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("softbuffer surface: {e}");
                event_loop.exit();
                return;
            }
        };
        self.state = Some(WinState {
            app,
            fb: None,
            window,
            surface,
            size: (size.width, size.height),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else { return };
        use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent as WE};
        match event {
            WE::CloseRequested => event_loop.exit(),
            WE::Resized(size) => state.size = (size.width, size.height),
            WE::RedrawRequested => state.present(),
            WE::CursorMoved { position, .. } => {
                state.app.set_pointer(position.x as f32, position.y as f32)
            }
            WE::CursorLeft { .. } => state.app.pointer_left(),
            WE::MouseInput { state: ms, button, .. } => {
                if button == MouseButton::Left {
                    state.app.set_down(ms == ElementState::Pressed);
                }
            }
            WE::Touch(touch) => {
                let (x, y) = (touch.location.x as f32, touch.location.y as f32);
                state.app.set_pointer(x, y);
                match touch.phase {
                    TouchPhase::Started => state.app.set_down(true),
                    TouchPhase::Moved => {}
                    TouchPhase::Ended | TouchPhase::Cancelled => state.app.set_down(false),
                }
            }
            WE::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    use winit::keyboard::{KeyCode, PhysicalKey};
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Space) => state.app.blink_now(),
                        PhysicalKey::Code(KeyCode::KeyT) => state.app.toggle_talk(),
                        PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(state) = self.state.as_mut() {
            state.window.request_redraw();
        }
    }
}

pub fn run_window(
    event_loop: winit::event_loop::EventLoop<()>,
) -> Result<(), winit::error::EventLoopError> {
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    event_loop.run_app(&mut Handler::default())
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: android_activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info));
    std::panic::set_hook(Box::new(|info| {
        log::error!("cubicraft panic: {info}");
    }));
    let event_loop = winit::event_loop::EventLoop::builder()
        .with_android_app(app)
        .build()
        .expect("event loop");
    if let Err(e) = run_window(event_loop) {
        log::error!("run error: {e}");
    }
}
