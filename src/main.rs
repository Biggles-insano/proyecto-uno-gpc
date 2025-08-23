mod map;
mod player;
mod raycaster;
mod render;

use minifb::{Key, Window, WindowOptions, MouseButton, MouseMode};
use std::f32::consts::PI;
use std::time::{Duration, Instant};
use map::Map;
use player::Player;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;

fn main() {
    // Framebuffer
    let mut buffer = vec![0x000000u32; WIDTH * HEIGHT];

    let mut window = Window::new(
        "Proyecto Uno - Ray Caster",
        WIDTH,
        HEIGHT,
        WindowOptions {
            resize: false,
            scale: minifb::Scale::X1,
            ..WindowOptions::default()
        },
    )
    .expect("No se pudo crear la ventana");

    window.limit_update_rate(Some(Duration::from_micros(1_000_000 / 60)));

    let map = Map::new();
    let mut player = Player::from_map_spawn(&map);

    let mut last_frame_time = Instant::now();

    // FPS
    let mut last_fps_update = Instant::now();
    let mut frame_count: u32 = 0;
    let mut fps: u32 = 0;
    let mut prev_mouse_x: Option<f32> = None;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Delta time (segundos)
        let now = Instant::now();
        let dt = now.duration_since(last_frame_time).as_secs_f32();
        last_frame_time = now;

        // Input movimiento/rotación (con colisión)
        if window.is_key_down(Key::W) { player.forward_collide(dt, &map); }
        if window.is_key_down(Key::S) { player.backward_collide(dt, &map); }
        if window.is_key_down(Key::A) { player.strafe_left_collide(dt, &map); }
        if window.is_key_down(Key::D) { player.strafe_right_collide(dt, &map); }
        if window.is_key_down(Key::Q) { player.turn_left(dt); }
        if window.is_key_down(Key::E) { player.turn_right(dt); }
        if window.is_key_down(Key::Left) { player.turn_left(dt); }
        if window.is_key_down(Key::Right) { player.turn_right(dt); }

        // Mouse drag-to-look (horizontal yaw while left button is pressed)
        if window.get_mouse_down(MouseButton::Left) {
            if let Some((mx, _my)) = window.get_mouse_pos(MouseMode::Pass) {
                if let Some(prev) = prev_mouse_x {
                    let dx = mx - prev;
                    // Sensibilidad (rad/píxel) – ajusta a gusto
                    let sensitivity: f32 = 0.004;
                    player.angle += dx as f32 * sensitivity;
                    // Normalizar ángulo a [-PI, PI)
                    while player.angle >= PI { player.angle -= 2.0 * PI; }
                    while player.angle < -PI { player.angle += 2.0 * PI; }
                }
                prev_mouse_x = Some(mx);
            } else {
                // No hay posición de mouse este frame
                prev_mouse_x = None;
            }
        } else {
            // Al soltar el clic, reseteamos para no acumular saltos
            prev_mouse_x = None;
        }

        // Render escena completa (cielo, suelo y paredes)
        render::draw_scene(&mut buffer, WIDTH, HEIGHT, &map, &player);

        // Actualiza FPS cada 1s + título con estado del jugador
        frame_count += 1;
        if last_fps_update.elapsed().as_secs_f32() >= 1.0 {
            fps = frame_count;
            frame_count = 0;
            last_fps_update = Instant::now();
            window.set_title(&format!(
                "Proyecto Uno - Ray Caster | {} FPS | x: {:.2} y: {:.2} angle: {:.2}°",
                fps, player.x, player.y, player.angle.to_degrees()
            ));
        }

        window
            .update_with_buffer(&buffer, WIDTH, HEIGHT)
            .expect("No se pudo actualizar el framebuffer");
    }
}