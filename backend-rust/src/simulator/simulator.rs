#![cfg(feature = "use_simulator")]

use std::sync::{Arc, Mutex};

use macroquad::miniquad::conf::Icon;
use macroquad::prelude::*;

use super::super::app_state::AppState;
use super::super::renderer::renderer::Renderer;

pub fn window_conf() -> Conf {
    Conf {
        window_title: "Nova Simulator".to_owned(),
        window_width: 720,
        window_height: 720,
        icon: Some(Icon {
            small: [128; 1024],
            medium: [128; 4096],
            big: [128; 16384],
        }),
        ..Default::default()
    }
}

pub async fn run_simulator(state: Arc<Mutex<AppState>>, renderer: Arc<Renderer>) {
    println!("Starting GPU simulator.");

    loop {
        clear_background(BLACK);

        set_camera(&Camera3D {
            position: vec3(-20., 15., 0.),
            up: vec3(0., 1., 0.),
            target: vec3(0., 0., 0.),
            ..Default::default()
        });

        draw_grid(20, 1., BLACK, GRAY);
        next_frame().await
    }
}
