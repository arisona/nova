use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use glam::{Mat3, vec3, vec4};
use miniquad::conf::Icon;
use miniquad::*;

use crate::check_run_once;

use crate::app_state::{AppState, Status};
use crate::renderer::{RenderState, Renderer};

const VOXEL_SPACING: f32 = 4.0;
const VOXEL_HALF_SIZE: f32 = 1.0;

pub fn run_simulator(state: Arc<Mutex<AppState>>, renderer: Renderer) {
    check_run_once!("Nova simulator already running.");

    log::info!("Starting Nova simulator.");

    state
        .lock()
        .unwrap()
        .set_status(Status::Ok("Nova simulator running.".to_string()));

    miniquad::start(conf(), move || Box::new(Stage::new(state, renderer)));
}

struct Stage {
    app_state: Arc<Mutex<AppState>>,
    renderer: Renderer,

    ctx: Box<dyn RenderingBackend>,

    pipeline: Pipeline,
    bindings: Bindings,

    instances: Vec<(f32, f32, f32, f32, f32, f32, f32)>,
    rot: f32,

    last_frame_time: std::time::Instant,
}

impl Stage {
    fn new(state: Arc<Mutex<AppState>>, renderer: Renderer) -> Self {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();

        let r = VOXEL_HALF_SIZE;
        #[rustfmt::skip]
        let vertices: &[f32] = &[
            -r, -r, 0.0,
             r, -r, 0.0,
             r,  r, 0.0,
            -r,  r, 0.0,
        ];
        // vertex buffer for static geometry
        let geometry_vertex_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(vertices),
        );

        #[rustfmt::skip]
        let indices: &[u32] = &[
            0, 1, 2,
            0, 2, 3,
        ];
        let index_buffer = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Immutable,
            BufferSource::slice(indices),
        );

        // empty, dynamic instance data vertex buffer
        const DX: usize = AppState::MODULE_X_RES * AppState::MODULE_GRID_MAX;
        const DY: usize = AppState::MODULE_Y_RES * AppState::MODULE_GRID_MAX;
        const DZ: usize = AppState::MODULE_Z_RES;
        let instance_vertex_buffer = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<u8>(28 * DX * DY * DZ),
        );

        let bindings = Bindings {
            vertex_buffers: vec![geometry_vertex_buffer, instance_vertex_buffer],
            index_buffer,
            images: vec![],
        };

        let shader = ctx
            .new_shader(
                match ctx.info().backend {
                    Backend::OpenGl => ShaderSource::Glsl {
                        vertex: shader::VERTEX,
                        fragment: shader::FRAGMENT,
                    },
                    Backend::Metal => ShaderSource::Msl {
                        program: shader::METAL,
                    },
                },
                shader::meta(),
            )
            .unwrap();

        let pipeline = ctx.new_pipeline(
            &[
                BufferLayout::default(),
                BufferLayout {
                    step_func: VertexStep::PerInstance,
                    ..Default::default()
                },
            ],
            &[
                VertexAttribute::with_buffer("in_pos", VertexFormat::Float3, 0),
                VertexAttribute::with_buffer("in_inst_pos", VertexFormat::Float3, 1),
                VertexAttribute::with_buffer("in_inst_color", VertexFormat::Float4, 1),
            ],
            shader,
            PipelineParams {
                depth_write: true,
                depth_test: Comparison::Less,
                ..Default::default()
            },
        );

        Stage {
            app_state: state,
            renderer,
            ctx,
            pipeline,
            bindings,
            instances: Vec::with_capacity(DX * DY * DZ),
            rot: 0.0,
            last_frame_time: std::time::Instant::now(),
        }
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {
        let now = std::time::Instant::now();
        self.last_frame_time = now;

        let rot = Mat3::from_rotation_y(self.rot);
        self.rot += 0.001;

        let (mut render_state, flip) = {
            let app_state = self.app_state.lock().unwrap();
            (RenderState::from(&app_state), app_state.is_flip_vertical())
        };
        self.renderer.render(&mut render_state);

        // create grid
        self.instances.clear();
        let (dx, dy, dz) = self.renderer.image().dim();
        for x in 0..dx {
            for y in 0..dy {
                for z in 0..dz {
                    let p = vec3(
                        x as f32 - (dx as f32 - 1.0) / 2.0,
                        y as f32 - (dy as f32 - 1.0) / 2.0,
                        z as f32 - (dz as f32 - 1.0) / 2.0,
                    );
                    // convert z-up to y-up and flip if needed
                    let p = vec3(p.x, if flip { p.z } else { -p.z }, -p.y);
                    let p = VOXEL_SPACING * rot * p;
                    let rgb = self.renderer.image().get(x, y, z);
                    let c = vec4(rgb.x, rgb.y, rgb.z, 1.0);
                    self.instances.push((p.x, p.y, p.z, c.x, c.y, c.z, c.w));
                }
            }
        }
    }

    fn draw(&mut self) {
        self.ctx.buffer_update(
            self.bindings.vertex_buffers[1],
            BufferSource::slice(&self.instances[..]),
        );

        // model-view-projection matrix
        let (width, height) = window::screen_size();
        let uniforms = camera_uniforms(self.renderer.image().dim(), width, height);

        self.ctx.begin_default_pass(Default::default());

        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&self.bindings);
        self.ctx.apply_uniforms(UniformsSource::table(&uniforms));
        self.ctx.draw(0, 6, self.instances.len() as i32);
        self.ctx.end_render_pass();

        self.ctx.commit_frame();
    }
}

fn camera_uniforms(dim: (usize, usize, usize), width: f32, height: f32) -> shader::Uniforms {
    let half_extents = vec3(
        dim.0.saturating_sub(1) as f32,
        dim.1.saturating_sub(1) as f32,
        dim.2.saturating_sub(1) as f32,
    ) * (VOXEL_SPACING / 2.0);
    let radius = half_extents.length() + std::f32::consts::SQRT_2 * VOXEL_HALF_SIZE;
    let aspect = width.max(1.0) / height.max(1.0);
    let vertical_fov = 60.0f32.to_radians();
    let limiting_half_fov = ((vertical_fov / 2.0).tan() * aspect.min(1.0)).atan();
    let distance = 1.05 * radius / limiting_half_fov.sin();
    let proj = glam::camera::rh::proj::opengl::perspective(
        vertical_fov,
        aspect,
        0.01,
        distance + 2.0 * radius,
    );
    let view = glam::camera::rh::view::look_at_mat4(
        vec3(0.0, 0.1, 1.0).normalize() * distance,
        vec3(0.0, 0.0, 0.0),
        vec3(0.0, 1.0, 0.0),
    );
    shader::Uniforms { view, proj }
}

fn conf() -> conf::Conf {
    conf::Conf {
        window_title: "Nova Simulator".to_string(),
        window_width: 1024,
        window_height: 768,
        icon: Some(Icon {
            small: [128; 16 * 16 * 4],
            medium: [128; 32 * 32 * 4],
            big: [128; 64 * 64 * 4],
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_fits_rotating_grids_and_voxel_quads() {
        let max_x = AppState::MODULE_X_RES * AppState::MODULE_GRID_MAX;
        let max_y = AppState::MODULE_Y_RES * AppState::MODULE_GRID_MAX;
        for dim in [
            (5, 5, 10),
            (max_x, 5, 10),
            (5, max_y, 10),
            (max_x, max_y, AppState::MODULE_Z_RES),
            (1, 1, 100),
            (1, 1, 1),
        ] {
            for (width, height) in [
                (1024.0, 768.0),
                (768.0, 1024.0),
                (1600.0, 400.0),
                (100.0, 1600.0),
                (800.0, 800.0),
                (0.0, 0.0),
            ] {
                let uniforms = camera_uniforms(dim, width, height);
                let transform = uniforms.proj * uniforms.view;
                for step in 0..24 {
                    let rotation =
                        Mat3::from_rotation_y(step as f32 * std::f32::consts::TAU / 24.0);
                    for grid_x in [0, dim.0 - 1] {
                        for grid_y in [0, dim.1 - 1] {
                            for grid_z in [0, dim.2 - 1] {
                                let center = VOXEL_SPACING
                                    * rotation
                                    * vec3(
                                        grid_x as f32 - (dim.0 - 1) as f32 / 2.0,
                                        grid_z as f32 - (dim.2 - 1) as f32 / 2.0,
                                        -(grid_y as f32 - (dim.1 - 1) as f32 / 2.0),
                                    );
                                for quad_x in [-VOXEL_HALF_SIZE, VOXEL_HALF_SIZE] {
                                    for quad_y in [-VOXEL_HALF_SIZE, VOXEL_HALF_SIZE] {
                                        let clip = transform
                                            * (center + vec3(quad_x, quad_y, 0.0)).extend(1.0);
                                        let ndc = clip.truncate() / clip.w;
                                        assert!(
                                            clip.w > 0.0 && ndc.abs().max_element() <= 1.0,
                                            "dim={dim:?}, viewport={width}x{height}, step={step}, ndc={ndc:?}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

mod shader {
    use miniquad::*;

    pub const VERTEX: &str = r#"#version 150
    in vec3 in_pos;
    in vec3 in_inst_pos;
    in vec4 in_inst_color;

    out vec4 color;
    out vec2 frag_uv;

    uniform mat4 view;
    uniform mat4 proj;

    void main() {
        vec4 pos = vec4(in_pos + in_inst_pos, 1.0);
        gl_Position = proj * view * pos;
        frag_uv = in_pos.xy * 0.5 + 0.5;
        color = in_inst_color;
    }
    "#;

    pub const FRAGMENT: &str = r#"#version 150
    in vec4 color;
    in vec2 frag_uv;

    out vec4 out_color;

    void main() {
        float dist = length(frag_uv - vec2(0.5, 0.5));
        if (dist > 0.5) discard;
        out_color = color;
    }
    "#;

    // metal currently not supported
    pub const METAL: &str = "";

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec![],
            uniforms: UniformBlockLayout {
                uniforms: vec![
                    UniformDesc::new("view", UniformType::Mat4),
                    UniformDesc::new("proj", UniformType::Mat4),
                ],
            },
        }
    }

    #[repr(C)]
    pub struct Uniforms {
        pub view: glam::Mat4,
        pub proj: glam::Mat4,
    }
}
