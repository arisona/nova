use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use glam::{Mat3, Mat4, Vec3, Vec4, vec3};
use miniquad::conf::Icon;
use miniquad::*;

use crate::check_run_once;

use crate::app_state::AppState;
use crate::renderer::renderer::{RenderState, Renderer};
use crate::renderer::voxel_image::VoxelImage;

pub fn run_simulator(state: Arc<Mutex<AppState>>, renderer: Renderer) {
    check_run_once!("Nova simulator already running.");

    println!("Starting Nova simulator.");

    state
        .lock()
        .unwrap()
        .set_status((true, "Nova simulator running."));

    miniquad::start(conf(), move || Box::new(Stage::new(state, renderer)));
}

struct Stage {
    app_state: Arc<Mutex<AppState>>,
    renderer: Renderer,
    image: VoxelImage,

    ctx: Box<dyn RenderingBackend>,

    pipeline: Pipeline,
    bindings: Bindings,

    instances: Vec<(f32, f32, f32, f32, f32, f32, f32)>,
    rot: f32,
    distance_factor: f32,

    last_frame_time: std::time::Instant,
}

impl Stage {
    pub fn new(state: Arc<Mutex<AppState>>, renderer: Renderer) -> Self {
        let mut ctx: Box<dyn RenderingBackend> = window::new_rendering_backend();

        let r = 1.0;
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

        let image = VoxelImage::new(state.lock().unwrap().dim());

        Stage {
            app_state: state,
            renderer,
            image,
            ctx,
            pipeline,
            bindings,
            instances: Vec::with_capacity(DX * DY * DZ),
            rot: 0.0,
            distance_factor: 1.0,
            last_frame_time: std::time::Instant::now(),
        }
    }
}

impl EventHandler for Stage {
    fn update(&mut self) {
        let now = std::time::Instant::now();
        let delta_time = now.duration_since(self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;

        let rot = Mat3::from_rotation_y(self.rot);
        self.rot += 0.001;

        let app_state = self.app_state.lock().unwrap();
        let flip = app_state.is_flip_vertical();
        let state = RenderState::from(&app_state);
        self.renderer.render(&state, &mut self.image, delta_time);

        // create grid
        let dx = self.image.dx();
        let dy = self.image.dy();
        let dz = self.image.dz();
        self.instances.clear();
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
                    let p = 4.0 * rot * p;
                    let rgb = self.image.get(x, y, z);
                    let c = Vec4 {
                        x: rgb.0,
                        y: rgb.1,
                        z: rgb.2,
                        w: 1.0,
                    };
                    self.instances.push((p.x, p.y, p.z, c.x, c.y, c.z, c.w));
                }
            }
        }
        self.distance_factor = (dx as f32).max(dy as f32) / 5.0;
    }

    fn draw(&mut self) {
        // by default glam-rs can vec3 as u128 or #[reprc(C)](f32, f32, f32).
        // need to ensure that the second option was used.
        assert_eq!(std::mem::size_of::<Vec3>(), 12);

        self.ctx.buffer_update(
            self.bindings.vertex_buffers[1],
            BufferSource::slice(&self.instances[..]),
        );

        // model-view-projection matrix
        let (width, height) = window::screen_size();

        let proj = Mat4::perspective_rh_gl(60.0f32.to_radians(), width / height, 0.01, 1000.0);
        let view = Mat4::look_at_rh(
            vec3(0.0, 5.0 * self.distance_factor, 50.0 * self.distance_factor),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
        );

        self.ctx.begin_default_pass(Default::default());

        self.ctx.apply_pipeline(&self.pipeline);
        self.ctx.apply_bindings(&self.bindings);
        self.ctx
            .apply_uniforms(UniformsSource::table(&shader::Uniforms { view, proj }));
        self.ctx.draw(0, 6, self.instances.len() as i32);
        self.ctx.end_render_pass();

        self.ctx.commit_frame();
    }
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
