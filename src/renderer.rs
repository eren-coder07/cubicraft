// CubiCraft — renderer.
// The Cubism-style draw step: one texture atlas, one pipeline, and the frame's
// art meshes submitted as textured, per-vertex-colored triangles (draw order =
// rig order). Uses macroquad's raw miniquad context for the textured pass.

use macroquad::prelude::*;
use macroquad::miniquad::{self, *};

use crate::deform::{EvalResult, ATLAS_W, ATLAS_H};
use crate::params::Params;

#[repr(C)]
struct Uniforms {
    screen: (f32, f32),
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
    col: [f32; 4],
}

pub struct Renderer {
    pipeline: Pipeline,
    vbuf: BufferId,
    ibuf: BufferId,
    bindings: Bindings,
    verts: Vec<Vertex>,
    idx: Vec<u16>,
}

const VERT: &str = r#"#version 100
attribute vec2 pos;
attribute vec2 uv;
attribute vec4 col;
uniform vec2 screen;
varying vec2 v_uv;
varying vec4 v_col;
void main() {
    gl_Position = vec4(pos.x * screen.x - 1.0, 1.0 - pos.y * screen.y, 0.0, 1.0);
    v_uv = uv;
    v_col = col;
}"#;

const FRAG: &str = r#"#version 100
precision mediump float;
varying vec2 v_uv;
varying vec4 v_col;
uniform sampler2D tex;
void main() {
    vec4 c = texture2D(tex, v_uv) * v_col;
    if (c.a < 0.004) { discard; }
    gl_FragColor = c;
}"#;

fn shader_meta() -> ShaderMeta {
    ShaderMeta {
        images: vec!["tex".to_string()],
        uniforms: UniformBlockLayout {
            uniforms: vec![UniformDesc::new("screen", UniformType::Float2)],
        },
    }
}

impl Renderer {
    pub unsafe fn new(ctx: &mut dyn RenderingBackend) -> Renderer {
        let shader = ctx
            .new_shader(
                ShaderSource::Glsl { vertex: VERT, fragment: FRAG },
                shader_meta(),
            )
            .expect("shader compile failed");
        let pipeline = ctx.new_pipeline(
            &[BufferLayout::default()],
            &[
                VertexAttribute::new("pos", VertexFormat::Float2),
                VertexAttribute::new("uv", VertexFormat::Float2),
                VertexAttribute::new("col", VertexFormat::Float4),
            ],
            shader,
            PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        );
        let vbuf = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Dynamic,
            BufferSource::empty::<f32>(8 * 8192),
        );
        let ibuf = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Dynamic,
            BufferSource::empty::<u16>(6 * 8192),
        );
        let white = ctx.new_texture_from_rgba8(1, 1, &[255, 255, 255, 255]);
        let bindings = Bindings {
            vertex_buffers: vec![vbuf],
            index_buffer: ibuf,
            images: vec![white],
        };
        Renderer { pipeline, vbuf, ibuf, bindings, verts: Vec::with_capacity(4096), idx: Vec::with_capacity(8192) }
    }

    pub fn set_atlas(&mut self, ctx: &mut dyn RenderingBackend, rt: &RenderTarget) {
        let tex = rt.texture.raw_miniquad_id();
        self.bindings.images[0] = tex;
    }

    /// Upload and draw all meshes (model units -> pixels via ppu/origin).
    pub fn draw_model(&mut self, ctx: &mut dyn RenderingBackend, ev: &EvalResult, ppu: f32, ox: f32, oy: f32) {
        self.verts.clear();
        self.idx.clear();
        for m in &ev.meshes {
            let base = self.verts.len() as u16;
            for (v, uv) in m.verts.iter().zip(m.uvs.iter()) {
                self.verts.push(Vertex {
                    pos: [ox + v.x * ppu, oy + v.y * ppu],
                    uv: [uv.x, uv.y],
                    col: [1.0, 1.0, 1.0, m.opacity],
                });
            }
            for i in &m.idx {
                self.idx.push(base + *i);
            }
        }
        if self.idx.is_empty() {
            return;
        }
        ctx.apply_pipeline(&self.pipeline);
        ctx.apply_bindings(&self.bindings);
        ctx.apply_uniforms(miniquad::UniformsSource::table(&Uniforms {
            screen: (2.0 / screen_width(), 2.0 / screen_height()),
        }));
        unsafe {
            let verts_ptr = self.verts.as_ptr() as *const f32;
            let n_floats = self.verts.len() * 8;
            ctx.buffer_update(
                self.vbuf,
                BufferSource::slice(std::slice::from_raw_parts(verts_ptr, n_floats)),
            );
            let idx_ptr = self.idx.as_ptr();
            ctx.buffer_update(
                self.ibuf,
                BufferSource::slice(std::slice::from_raw_parts(idx_ptr, self.idx.len())),
            );
        }
        ctx.draw(0, self.idx.len() as i32, 1);
        ctx.end_render_pass();
    }
}

/// Bake all parts into the atlas render target with macroquad primitives.
pub fn bake_atlas_to_rt(parts: &[crate::art::Part], bake_fn: impl FnOnce()) -> RenderTarget {
    let rt = render_target(ATLAS_W as u32, ATLAS_H as u32);
    rt.texture.set_filter(FilterMode::Linear);
    let cam = Camera2D {
        zoom: vec2(2.0 / ATLAS_W as f32, -2.0 / ATLAS_H as f32),
        target: vec2(ATLAS_W as f32 / 2.0, ATLAS_H as f32 / 2.0),
        render_target: Some(rt.clone()),
        ..Default::default()
    };
    set_camera(&cam);
    clear_background(Color::new(0.0, 0.0, 0.0, 0.0));
    bake_fn();
    set_default_camera();
    rt
}

pub fn set_atlas_texture(rt: &RenderTarget) {
    let _ = rt;
}

pub fn ppu_for_view(w: f32, h: f32) -> f32 {
    (h / 2.9).min(w / 3.1)
}

pub fn world_to_screen(p: Vec2, ppu: f32, ox: f32, oy: f32) -> Vec2 {
    vec2(ox + p.x * ppu, oy + p.y * ppu)
}

pub fn screen_to_world(p: Vec2, ppu: f32, ox: f32, oy: f32) -> Vec2 {
    vec2((p.x - ox) / ppu, (p.y - oy) / ppu)
}

pub fn unused(_p: &Params) {}
