mod map;
mod player;
mod raycaster;
mod render;

use minifb::{Key, Window, WindowOptions, MouseButton, MouseMode};
use std::f32::consts::PI;
use std::time::{Duration, Instant};
use std::fs::File;
use std::io::BufReader;
use rodio::{OutputStream, OutputStreamHandle, Sink, Decoder, Source};
use map::Map;
use player::Player;

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const SWITCH_SECONDS: f32 = 5.0; // intervalo de cambio de mapa
const OBJ_SWITCH_SECONDS: f32 = 3.0; // intervalo para evaluar si el objetivo cambia (desacoplado del cambio de mapa)
const BGM_PATH: &str = "assets/music/clown_loop.ogg";
const VICTORY_SFX_PATH: &str = "assets/music/victory_fanfare.ogg";
const TP_SFX_PATH: &str = "assets/sfx/tp_pop.ogg";
const BGM_VOLUME: f32 = 0.35;
const SFX_VOLUME: f32 = 1.0;

#[derive(Copy, Clone, PartialEq, Eq)]
enum GameState {
    Menu,
    Playing,
    Victory,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum GameMode { Normal, Dificil }

fn compute_anchors(map: &Map) -> Vec<(f32, f32)> {
    let w = map.width() as i32;
    let h = map.height() as i32;
    if w < 4 || h < 4 { return Vec::new(); }
    let targets = [
        (w / 4, h / 4),
        (3 * w / 4, h / 4),
        (w / 4, 3 * h / 4),
        (3 * w / 4, 3 * h / 4),
    ];
    let mut out = Vec::new();
    for (mut tx, mut ty) in targets {
        tx = tx.clamp(1, w - 2);
        ty = ty.clamp(1, h - 2);
        if let Some((cx, cy)) = find_nearest_free_cell(map, tx, ty, 8) {
            if let Some((wx, wy)) = map.cell_center_world(cx, cy) { out.push((wx, wy)); }
        }
    }
    if out.is_empty() {
        if let Some((wx, wy)) = map.cell_center_world(1, 1) { out.push((wx, wy)); }
    }
    out
}

fn find_nearest_free_cell(map: &Map, cx: i32, cy: i32, max_r: i32) -> Option<(i32, i32)> {
    if cx >= 0 && cy >= 0 && !map.is_wall(cx, cy) { return Some((cx, cy)); }
    for r in 1..=max_r {
        // anillo superior e inferior
        for dx in -r..=r {
            let x = cx + dx;
            let y_top = cy - r;
            let y_bot = cy + r;
            if map.in_bounds(x, y_top) && !map.is_wall(x, y_top) { return Some((x, y_top)); }
            if map.in_bounds(x, y_bot) && !map.is_wall(x, y_bot) { return Some((x, y_bot)); }
        }
        // lados izquierdo y derecho (sin esquinas duplicadas)
        for dy in (-r + 1)..=r - 1 {
            let y = cy + dy;
            let x_left = cx - r;
            let x_right = cx + r;
            if map.in_bounds(x_left, y) && !map.is_wall(x_left, y) { return Some((x_left, y)); }
            if map.in_bounds(x_right, y) && !map.is_wall(x_right, y) { return Some((x_right, y)); }
        }
    }
    None
}

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

    // === Audio: stream y sinks
    let mut audio_stream: Option<OutputStream> = None;
    let mut audio_handle: Option<OutputStreamHandle> = None;
    let mut bgm_sink: Option<Sink> = None;
    let mut sfx_sink: Option<Sink> = None;
    if let Ok((stream, handle)) = OutputStream::try_default() {
        audio_stream = Some(stream); // mantener vivo
        audio_handle = Some(handle);
    }
    if let Some(handle) = audio_handle.as_ref() {
        if let Ok(s) = Sink::try_new(handle) { s.set_volume(BGM_VOLUME); bgm_sink = Some(s); }
        if let Ok(s) = Sink::try_new(handle) { s.set_volume(SFX_VOLUME); sfx_sink = Some(s); }
    }

    // Estado del juego
    let mut state = GameState::Menu;

    // Modo de juego y selección de menú
    let mut game_mode = GameMode::Dificil;
    let mut menu_selected: usize = 1; // 0 = Normal, 1 = Dificil

    // Anclas del objetivo (para modo Normal)
    let mut anchors: Vec<(f32, f32)> = Vec::new();
    let mut anchor_idx: Option<usize> = None;

    // Variantes de mapa por semilla
    let seeds: [u32; 3] = [0, 1, 2];
    let mut active_seed_idx: usize = 0;

    // Mundo/Jugador
    let mut map = Map::new_with_seed(seeds[active_seed_idx]);
    let mut player = Player::from_map_spawn(&map);

    // Objetivo (coleccionable)
    let (mut obj_x, mut obj_y) = map.objective_world();
    let mut objective_found = false;

    let mut last_frame_time = Instant::now();

    // FPS
    let mut last_fps_update = Instant::now();
    let mut frame_count: u32 = 0;
    let mut fps: u32 = 0;
    let mut prev_mouse_x: Option<f32> = None;
    let mut anim_t: f32 = 0.0;

    // Temporizador de cambio de mapa
    let mut last_switch = Instant::now();
    let mut last_obj_check = Instant::now();
    let mut rng_state: u32 = 0xA36E_2D4F ^ seeds[active_seed_idx];

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // Delta time 
        let now = Instant::now();
        let dt = now.duration_since(last_frame_time).as_secs_f32();
        last_frame_time = now;
        anim_t += dt;

        match state {
            GameState::Menu => {
                // Limpia el buffer a negro
                for px in buffer.iter_mut() { *px = 0x000000; }

                // Dibuja menú con botón seleccionado
                render::draw_menu(&mut buffer, WIDTH, HEIGHT, menu_selected);

                // Navegación de botones (izq/der)
                if window.is_key_pressed(Key::Left, minifb::KeyRepeat::No) {
                    if menu_selected > 0 { menu_selected -= 1; }
                }
                if window.is_key_pressed(Key::Right, minifb::KeyRepeat::No) {
                    if menu_selected < 1 { menu_selected += 1; }
                }

                // Enter para jugar
                if window.is_key_pressed(Key::Enter, minifb::KeyRepeat::No) {
                    // Modo según selección actual del menú
                    game_mode = if menu_selected == 0 { GameMode::Normal } else { GameMode::Dificil };

                    active_seed_idx = 0;
                    map = Map::new_with_seed(seeds[active_seed_idx]);
                    player = Player::from_map_spawn(&map);

                    // Init RNG y temporizador del objetivo antes de colocarlo
                    last_obj_check = Instant::now();
                    rng_state = 0xA36E_2D4F ^ seeds[active_seed_idx];
                    if rng_state == 0 { rng_state = 0xB5297A4D; }

                    // Colocar objetivo según modo
                    match game_mode {
                        GameMode::Normal => {
                            anchors = compute_anchors(&map);
                            anchor_idx = None;
                            if !anchors.is_empty() {
                                rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                let idx = (rng_state as usize) % anchors.len();
                                let (wx, wy) = anchors[idx];
                                obj_x = wx; obj_y = wy; anchor_idx = Some(idx);
                            }
                        }
                        GameMode::Dificil => {
                            // Colocar objetivo en celda libre aleatoria 
                            let (pcx, pcy) = map.world_to_cell(player.x, player.y);
                            let mut placed = false;
                            for _ in 0..1024 {
                                // rand X
                                rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                if rng_state == 0 { rng_state = 0xB5297A4D; }
                                let rx = (rng_state as usize) % (map.width() - 2) + 1;
                                // rand Y
                                rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                if rng_state == 0 { rng_state = 0xB5297A4D; }
                                let ry = (rng_state as usize) % (map.height() - 2) + 1;
                                let cx = rx as i32; let cy = ry as i32;
                                if map.is_free(cx, cy) && !(cx == pcx && cy == pcy) {
                                    if let Some((wx, wy)) = map.cell_center_world(cx, cy) { obj_x = wx; obj_y = wy; placed = true; break; }
                                }
                            }
                            if !placed {
                                'outer: for y in 1..(map.height() as i32 - 1) {
                                    for x in 1..(map.width() as i32 - 1) {
                                        if map.is_free(x, y) && !(x == pcx && y == pcy) {
                                            if let Some((wx, wy)) = map.cell_center_world(x, y) { obj_x = wx; obj_y = wy; break 'outer; }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Música de fondo: arrancar loop 
                    if let Some(sink) = bgm_sink.as_ref() {
                        if sink.empty() { // no hay nada encolado aún
                            if let Ok(file) = File::open(BGM_PATH) {
                                if let Ok(dec) = Decoder::new(BufReader::new(file)) {
                                    sink.append(dec.repeat_infinite());
                                }
                            }
                            sink.set_volume(BGM_VOLUME);
                        }
                    }
                    objective_found = false;
                    state = GameState::Playing;
                    last_switch = Instant::now();
                }

                // Click sobre los botones para jugar
                if window.get_mouse_down(MouseButton::Left) {
                    if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Pass) {
                        let (r1, r2) = render::menu_button_rects(WIDTH, HEIGHT);
                        let in_rect = |r: (usize, usize, usize, usize), mx: f32, my: f32| -> bool {
                            let (x, y, w, h) = r;
                            mx >= x as f32 && mx < (x + w) as f32 && my >= y as f32 && my < (y + h) as f32
                        };
                        let clicked = if in_rect(r1, mx, my) { Some(0) } else if in_rect(r2, mx, my) { Some(1) } else { None };
                        if let Some(idx) = clicked {
                            menu_selected = idx;
                            game_mode = if menu_selected == 0 { GameMode::Normal } else { GameMode::Dificil };

                            active_seed_idx = 0;
                            map = Map::new_with_seed(seeds[active_seed_idx]);
                            player = Player::from_map_spawn(&map);
                            // Init RNG y temporizador
                            last_obj_check = Instant::now();
                            rng_state = 0xA36E_2D4F ^ seeds[active_seed_idx];
                            if rng_state == 0 { rng_state = 0xB5297A4D; }

                            // Colocar objetivo según modo
                            match game_mode {
                                GameMode::Normal => {
                                    anchors = compute_anchors(&map);
                                    anchor_idx = None;
                                    if !anchors.is_empty() {
                                        rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                        let idx = (rng_state as usize) % anchors.len();
                                        let (wx, wy) = anchors[idx];
                                        obj_x = wx; obj_y = wy; anchor_idx = Some(idx);
                                    }
                                }
                                GameMode::Dificil => {
                                    let (pcx, pcy) = map.world_to_cell(player.x, player.y);
                                    let mut placed = false;
                                    for _ in 0..1024 {
                                        rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                        if rng_state == 0 { rng_state = 0xB5297A4D; }
                                        let rx = (rng_state as usize) % (map.width() - 2) + 1;
                                        rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                        if rng_state == 0 { rng_state = 0xB5297A4D; }
                                        let ry = (rng_state as usize) % (map.height() - 2) + 1;
                                        let cx = rx as i32; let cy = ry as i32;
                                        if map.is_free(cx, cy) && !(cx == pcx && cy == pcy) {
                                            if let Some((wx, wy)) = map.cell_center_world(cx, cy) { obj_x = wx; obj_y = wy; placed = true; break; }
                                        }
                                    }
                                    if !placed {
                                        'outer: for y in 1..(map.height() as i32 - 1) {
                                            for x in 1..(map.width() as i32 - 1) {
                                                if map.is_free(x, y) && !(x == pcx && y == pcy) {
                                                    if let Some((wx, wy)) = map.cell_center_world(x, y) { obj_x = wx; obj_y = wy; break 'outer; }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Música de fondo
                            if let Some(sink) = bgm_sink.as_ref() {
                                if sink.empty() {
                                    if let Ok(file) = File::open(BGM_PATH) {
                                        if let Ok(dec) = Decoder::new(BufReader::new(file)) {
                                            sink.append(dec.repeat_infinite());
                                        }
                                    }
                                    sink.set_volume(BGM_VOLUME);
                                }
                            }

                            objective_found = false;
                            state = GameState::Playing;
                            last_switch = Instant::now();
                        }
                    }
                }

                // Título (instrucciones)
                if last_fps_update.elapsed().as_secs_f32() >= 0.5 {
                    window.set_title("Menú — Clic en JUGAR o ENTER");
                    last_fps_update = Instant::now();
                }

                // No mouse-look en menú
                prev_mouse_x = None;
            }
            GameState::Playing => {
                // Reubicación del objetivo con probabilidad 50% cada OBJ_SWITCH_SECONDS
                if !objective_found && last_obj_check.elapsed().as_secs_f32() >= OBJ_SWITCH_SECONDS {
                    let mut did_teleport = false;
                    // xorshift32 determinista
                    rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                    let coin = rng_state & 1; // 0 o 1 con ~50%
                    if rng_state == 0 { rng_state = 0x1B873593; }
                    if coin == 1 {
                        match game_mode {
                            GameMode::Normal => {
                                // Elegir una ancla distinta a la actual
                                if !anchors.is_empty() {
                                    let cur = anchor_idx.unwrap_or(usize::MAX);
                                    let mut tries = 0;
                                    let mut next = cur;
                                    while tries < 8 {
                                        rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                        let cand = (rng_state as usize) % anchors.len();
                                        if cand != cur { next = cand; break; }
                                        tries += 1;
                                    }
                                    if next == usize::MAX { next = 0; }
                                    let (wx, wy) = anchors[next];
                                    obj_x = wx; obj_y = wy; anchor_idx = Some(next); did_teleport = true;
                                }
                            }
                            GameMode::Dificil => {
                                let (ocx, ocy) = map.world_to_cell(obj_x, obj_y);
                                // Teletransportar a cualquier celda libre del mapa (sin restricción de distancia)
                                let mut placed = false;
                                for _ in 0..1024 {
                                    // rand para X
                                    rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                    let rx = (rng_state as usize) % (map.width() - 2) + 1;
                                    // rand para Y
                                    rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                    let ry = (rng_state as usize) % (map.height() - 2) + 1;
                                    let cx = rx as i32; let cy = ry as i32;
                                    if cx == ocx || cy == ocy { continue; }
                                    if map.is_free(cx, cy) {
                                        if let Some((wx, wy)) = map.cell_center_world(cx, cy) { obj_x = wx; obj_y = wy; placed = true; did_teleport = true; break; }
                                    }
                                }
                                if !placed {
                                    // Fallback: barrido determinista buscando la primera celda libre
                                    'outer: for y in 1..(map.height() as i32 - 1) {
                                        for x in 1..(map.width() as i32 - 1) {
                                            if map.is_free(x, y) && x != ocx && y != ocy {
                                                if let Some((wx, wy)) = map.cell_center_world(x, y) { obj_x = wx; obj_y = wy; did_teleport = true; break 'outer; }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if did_teleport {
                        if let Some(sink) = sfx_sink.as_ref() {
                            if let Ok(file) = File::open(TP_SFX_PATH) {
                                if let Ok(dec) = Decoder::new(BufReader::new(file)) {
                                    sink.append(dec);
                                    sink.set_volume(SFX_VOLUME);
                                }
                            }
                        }
                    }
                    last_obj_check = Instant::now();
                }

                // Cambio de mapa cada SWITCH_SECONDS
                if last_switch.elapsed().as_secs_f32() >= SWITCH_SECONDS {
                    active_seed_idx = (active_seed_idx + 1) % seeds.len();
                    let new_map = Map::new_with_seed(seeds[active_seed_idx]);

                    // Recolocación segura si la celda actual pasa a ser muro
                    let (cx, cy) = new_map.world_to_cell(player.x, player.y);
                    if new_map.is_wall(cx, cy) {
                        if let Some((fx, fy)) = find_nearest_free_cell(&new_map, cx, cy, 6) {
                            if let Some((wx, wy)) = new_map.cell_center_world(fx, fy) {
                                player.x = wx; player.y = wy;
                            }
                        } else {
                            // Como fallback, usa el spawn recomendado
                            let (wx, wy) = new_map.recommended_spawn();
                            player.x = wx; player.y = wy;
                        }
                    }

                    // Nuevo objetivo para la nueva variante
                    map = new_map;
                    // Reposicionar objetivo según modo para la nueva variante
                    match game_mode {
                        GameMode::Normal => {
                            anchors = compute_anchors(&map);
                            anchor_idx = None;
                            if !anchors.is_empty() {
                                rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                let idx = (rng_state as usize) % anchors.len();
                                let (wx, wy) = anchors[idx];
                                obj_x = wx; obj_y = wy; anchor_idx = Some(idx);
                            }
                        }
                        GameMode::Dificil => {
                            // Colocar objetivo en celda libre aleatoria (evita la celda del jugador)
                            let (pcx, pcy) = map.world_to_cell(player.x, player.y);
                            let mut placed = false;
                            for _ in 0..1024 {
                                rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                if rng_state == 0 { rng_state = 0x68E31DA4; }
                                let rx = (rng_state as usize) % (map.width() - 2) + 1;
                                rng_state ^= rng_state << 13; rng_state ^= rng_state >> 17; rng_state ^= rng_state << 5;
                                if rng_state == 0 { rng_state = 0x68E31DA4; }
                                let ry = (rng_state as usize) % (map.height() - 2) + 1;
                                let cx = rx as i32; let cy = ry as i32;
                                if map.is_free(cx, cy) && !(cx == pcx && cy == pcy) {
                                    if let Some((wx, wy)) = map.cell_center_world(cx, cy) { obj_x = wx; obj_y = wy; placed = true; break; }
                                }
                            }
                            if !placed {
                                'outer: for y in 1..(map.height() as i32 - 1) {
                                    for x in 1..(map.width() as i32 - 1) {
                                        if map.is_free(x, y) && !(x == pcx && y == pcy) {
                                            if let Some((wx, wy)) = map.cell_center_world(x, y) { obj_x = wx; obj_y = wy; break 'outer; }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Sonido de teletransporte al reubicar por cambio de mapa
                    if let Some(sink) = sfx_sink.as_ref() {
                        if let Ok(file) = File::open(TP_SFX_PATH) {
                            if let Ok(dec) = Decoder::new(BufReader::new(file)) {
                                sink.append(dec);
                                sink.set_volume(SFX_VOLUME);
                            }
                        }
                    }
                    objective_found = false;
                    last_obj_check = Instant::now(); rng_state ^= seeds[active_seed_idx] ^ 0x9E3779B1; if rng_state == 0 { rng_state = 0x68E31DA4; }
                    last_switch = Instant::now();
                }

                // Input movimiento/rotación 
                if window.is_key_down(Key::W) { player.forward_collide(dt, &map); }
                if window.is_key_down(Key::S) { player.backward_collide(dt, &map); }
                if window.is_key_down(Key::A) { player.strafe_left_collide(dt, &map); }
                if window.is_key_down(Key::D) { player.strafe_right_collide(dt, &map); }
                if window.is_key_down(Key::Q) { player.turn_left(dt); }
                if window.is_key_down(Key::E) { player.turn_right(dt); }
                if window.is_key_down(Key::Left) { player.turn_left(dt); }
                if window.is_key_down(Key::Right) { player.turn_right(dt); }

                // Mouse drag-to-look mientras está presionado el botón izquierdo
                if window.get_mouse_down(MouseButton::Left) {
                    if let Some((mx, _my)) = window.get_mouse_pos(MouseMode::Pass) {
                        if let Some(prev) = prev_mouse_x {
                            let dx = mx - prev;
                            let sensitivity: f32 = 0.004;
                            player.angle += dx as f32 * sensitivity;
                            while player.angle >= PI { player.angle -= 2.0 * PI; }
                            while player.angle < -PI { player.angle += 2.0 * PI; }
                        }
                        prev_mouse_x = Some(mx);
                    } else {
                        prev_mouse_x = None;
                    }
                } else {
                    prev_mouse_x = None;
                }

                // Detección de recogida del objetivo (radio amplio ~0.7 * TILE_SIZE para "atravesarlo")
                if !objective_found {
                    let dx = player.x - obj_x;
                    let dy = player.y - obj_y;
                    let dist2 = dx * dx + dy * dy;
                    let pick_r = map.tile_size() as f32 * 0.7;
                    if dist2 <= pick_r * pick_r {
                        if let Some(sink) = bgm_sink.as_ref() { sink.set_volume(BGM_VOLUME * 0.2); }
                        if let Some(sink) = sfx_sink.as_ref() {
                            if let Ok(file) = File::open(VICTORY_SFX_PATH) {
                                if let Ok(dec) = Decoder::new(BufReader::new(file)) { sink.append(dec); }
                            }
                        }
                        objective_found = true;
                        state = GameState::Victory;
                        window.set_title("¡Victoria! — ENTER para volver al menú");
                    }
                }

                // Render escena completa + minimapa
                render::draw_scene(&mut buffer, WIDTH, HEIGHT, &map, &player, obj_x, obj_y, anim_t);
                render::draw_minimap(&mut buffer, WIDTH, HEIGHT, &map, &player, obj_x, obj_y, anim_t);
                render::draw_fps_hud(&mut buffer, WIDTH, HEIGHT, fps);

                // Actualiza FPS cada 1s + título (incluye estado del objetivo y distancia)
                frame_count += 1;
                if last_fps_update.elapsed().as_secs_f32() >= 1.0 {
                    fps = frame_count;
                    frame_count = 0;
                    last_fps_update = Instant::now();

                    let dx = player.x - obj_x;
                    let dy = player.y - obj_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let obj_txt = if objective_found { "OBJ: 1/1" } else { "OBJ: 0/1" };

                    window.set_title(&format!(
                        "Proyecto Uno - Ray Caster | {} FPS | seed:{} | {} | dist:{:.0} | x:{:.1} y:{:.1} ang:{:.1}°",
                        fps, map.seed(), obj_txt, dist, player.x, player.y, player.angle.to_degrees()
                    ));
                }
            }
            GameState::Victory => {
                // Mostrar pantalla de victoria; no hay input de juego ni cambio de mapa
                for px in buffer.iter_mut() { *px = 0x000000; }
                render::draw_victory(&mut buffer, WIDTH, HEIGHT);

                // Volver al menú
                if window.is_key_pressed(Key::Enter, minifb::KeyRepeat::No) || window.get_mouse_down(MouseButton::Left) {
                    state = GameState::Menu;
                    window.set_title("Menú — Clic en JUGAR o ENTER");
                }
            }
        }

        window
            .update_with_buffer(&buffer, WIDTH, HEIGHT)
            .expect("No se pudo actualizar el framebuffer");
    }
}