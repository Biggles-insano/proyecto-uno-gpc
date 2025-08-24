use crate::map::{Map, TILE_SIZE};
use crate::player::Player;
use crate::raycaster::{self, RayHit};
use std::f32::consts::PI;

const SKY: u32 = 0x00D5FF;   // cyan eléctrico
const FLOOR: u32 = 0x1E1B2E; // púrpura muy oscuro
const OBJ_COLOR: u32 = 0xFF2ED1; // magenta brillante del objetivo (sprite 3D)

// Colores por ID de pared (ajustables luego)
fn wall_color(id: u8) -> u32 {
    match id {
        1 => 0xFF6EC7, // rosa intenso
        2 => 0xFFA500, // naranja vivo
        3 => 0x00FF88, // verde neón
        4 => 0x6A5CFF, // violeta eléctrico
        _ => 0xFFFFFF, // blanco por defecto
    }
}

fn shade(color: u32, factor: f32) -> u32 {
    // factor en [0..1], multiplica canales RGB linealmente
    let r = ((color >> 16) & 0xFF) as f32 * factor;
    let g = ((color >> 8) & 0xFF) as f32 * factor;
    let b = (color & 0xFF) as f32 * factor;
    ((r.clamp(0.0, 255.0) as u32) << 16)
        | ((g.clamp(0.0, 255.0) as u32) << 8)
        | (b.clamp(0.0, 255.0) as u32)
}

// ====== NEÓN ANIMADO (helpers a nivel de módulo) ======
fn neon_from_phase(phase: f32) -> u32 {
    // Paleta neón animada con senoides desfasadas 120°
    let base = 0.35; // brillo mínimo
    let amp  = 0.65; // amplitud
    let r = (base + amp * (phase).sin().mul_add(0.5, 0.5)).clamp(0.0, 1.0);
    let g = (base + amp * (phase + 2.0943951).sin().mul_add(0.5, 0.5)).clamp(0.0, 1.0);
    let b = (base + amp * (phase + 4.1887902).sin().mul_add(0.5, 0.5)).clamp(0.0, 1.0);
    let ri = (r * 255.0) as u32;
    let gi = (g * 255.0) as u32;
    let bi = (b * 255.0) as u32;
    (ri << 16) | (gi << 8) | bi
}

fn wall_color_anim(id: u8, t: f32) -> u32 {
    let phase = t * 0.6 + (id as f32) * 1.3; // cada ID con fase distinta
    neon_from_phase(phase)
}

#[inline]
fn put_pixel(buffer: &mut [u32], w: usize, h: usize, x: usize, y: usize, color: u32) {
    if x < w && y < h {
        buffer[y * w + x] = color;
    }
}


const MM_BG: u32 = 0x121212;      // fondo más profundo
const MM_WALL: u32 = 0xFAFAFA;    // paredes más contrastadas
const MM_PLAYER: u32 = 0x00FFFF;  // cian neón (igual)
const MM_BORDER: u32 = 0x606060;  // borde un poco más claro
const MM_OBJECTIVE: u32 = 0xFF00FF;  // objetivo magenta vivo

#[inline]
fn draw_rect(buffer: &mut [u32], w: usize, h: usize, x: usize, y: usize, rw: usize, rh: usize, color: u32) {
    let x2 = (x + rw).min(w);
    let y2 = (y + rh).min(h);
    for yy in y..y2 {
        let row = yy * w;
        for xx in x..x2 {
            buffer[row + xx] = color;
        }
    }
}

#[inline]
fn draw_line(buffer: &mut [u32], w: usize, h: usize, x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    // Bresenham sencillo
    let (mut x0, mut y0, mut x1, mut y1) = (x0, y0, x1, y1);
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        if x0 >= 0 && y0 >= 0 && (x0 as usize) < w && (y0 as usize) < h {
            buffer[y0 as usize * w + x0 as usize] = color;
        }
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x0 += sx; }
        if e2 <= dx { err += dx; y0 += sy; }
    }
}

#[inline]
fn draw_block(buffer: &mut [u32], w: usize, h: usize, x: usize, y: usize, scale: usize, color: u32) {
    draw_rect(buffer, w, h, x, y, scale, scale, color);
}

// ====== TEXTO 5x7 (bitmap mínimo para menú) ======
const TEXT_COLOR: u32 = 0xDDDDDD;
const TEXT_SHADOW: u32 = 0x060606;

/// Devuelve un glifo 5x7 **por fila** (5 filas útiles), cada u8 codifica 5 bits de izquierda a derecha.
fn glyph5x7(ch: char) -> [u8; 5] {
    match ch {
        'A' => [0b01110, 0b10001, 0b11111, 0b10001, 0b10001],
        'C' => [0b01110, 0b10001, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b11110, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000],
        'G' => [0b01110, 0b10000, 0b10111, 0b10001, 0b01110],
        'I' => [0b11111, 0b00100, 0b00100, 0b00100, 0b11111],
        'J' => [0b00111, 0b00010, 0b00010, 0b10010, 0b01100],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b11110, 0b10000, 0b10000],
        'R' => [0b11110, 0b10001, 0b11110, 0b10100, 0b10010],
        'S' => [0b11111, 0b10000, 0b11110, 0b00001, 0b11110],
        '0' => [0b01110, 0b10001, 0b10001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00010, 0b00100, 0b11111],
        '3' => [0b11110, 0b00001, 0b01110, 0b00001, 0b11110],
        '4' => [0b10001, 0b10001, 0b11111, 0b00001, 0b00001],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b11110],
        '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b00100],
        '8' => [0b01110, 0b10001, 0b01110, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b01111, 0b00001, 0b01110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'Y' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100],
        'W' => [0b10001, 0b10001, 0b10101, 0b10101, 0b01010],
        '!' => [0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        ' ' => [0, 0, 0, 0, 0],
        _   => [0, 0, 0, 0, 0], // fallback vacío
    }
}

#[inline]
fn draw_char5x7(buffer: &mut [u32], w: usize, h: usize, x: usize, y: usize, ch: char, scale: usize, color: u32) {
    let rows = glyph5x7(ch);
    // Interpretamos `rows` como 5 FILAS (alto), cada una con 5 bits (ancho)
    for (ry, bits) in rows.iter().enumerate() { // ry: 0..5
        for cx in 0..5 { // cx: 0..5
            // bit más significativo = columna izquierda
            let mask = 1 << (4 - cx);
            if (bits & mask) != 0 {
                draw_rect(buffer, w, h, x + cx * scale, y + ry * scale, scale, scale, color);
            }
        }
    }
}

#[inline]
fn draw_text5x7(buffer: &mut [u32], w: usize, h: usize, mut x: usize, y: usize, text: &str, scale: usize, color: u32) {
    let cw = 5 * scale; // ancho glifo
    let sp = 1 * scale; // espacio
    for ch in text.chars() {
        let ch_up = ch.to_ascii_uppercase();
        draw_char5x7(buffer, w, h, x, y, ch_up, scale, color);
        x += cw + sp;
    }
}

#[inline]
fn text_width5x7(text: &str, scale: usize) -> usize { text.chars().count() * (5 * scale + 1 * scale) - 1 * scale }

#[inline]
fn draw_text_centered5x7(buffer: &mut [u32], w: usize, h: usize, cx: usize, y: usize, text: &str, scale: usize, color: u32) {
    let tw = text_width5x7(text, scale);
    let x = cx.saturating_sub(tw / 2);
    draw_text5x7(buffer, w, h, x, y, text, scale, color);
}

/// Dibuja un minimapa en la esquina superior izquierda.
pub fn draw_minimap(buffer: &mut [u32], screen_w: usize, screen_h: usize, map: &Map, player: &Player, obj_x: f32, obj_y: f32, anim_t: f32) {
    // Tamaño máximo del minimapa (no más de ~1/3 del ancho ni 1/3 del alto)
    let max_w = screen_w / 3;
    let max_h = screen_h / 3;
    // Escala por celda (px) calculada dinámicamente, mínimo 1
    let scale_w = (max_w / map.width().max(1)).max(1);
    let scale_h = (max_h / map.height().max(1)).max(1);
    let scale = scale_w.min(scale_h).max(1);

    let margin = 8usize;
    let mut mm_w = map.width() * scale;
    let mut mm_h = map.height() * scale;

    // Si el minimapa es demasiado grande, recórtalo a un tope razonable
    mm_w = mm_w.min(max_w);
    mm_h = mm_h.min(max_h);

    // Fondo y borde
    draw_rect(buffer, screen_w, screen_h, margin, margin, mm_w, mm_h, MM_BG);
    // Borde (1px)
    // Top & bottom
    for x in margin..(margin + mm_w) {
        put_pixel(buffer, screen_w, screen_h, x, margin, MM_BORDER);
        if margin + mm_h - 1 < screen_h { put_pixel(buffer, screen_w, screen_h, x, margin + mm_h - 1, MM_BORDER); }
    }
    // Left & right
    for y in margin..(margin + mm_h) {
        put_pixel(buffer, screen_w, screen_h, margin, y, MM_BORDER);
        if margin + mm_w - 1 < screen_w { put_pixel(buffer, screen_w, screen_h, margin + mm_w - 1, y, MM_BORDER); }
    }

    // Dibuja paredes según el grid. Convertimos cada celda a bloque de `scale x scale`.
    // Nota: si el minimapa fue recortado por tope, ajustamos el número de celdas visibles.
    let cells_x = (mm_w / scale).min(map.width());
    let cells_y = (mm_h / scale).min(map.height());

    for cy in 0..cells_y {
        for cx in 0..cells_x {
            if map.is_wall(cx as i32, cy as i32) {
                let x = margin + cx * scale;
                let y = margin + cy * scale;
                // Fase por celda para variedad visual sin leer el ID
                let phase = anim_t * 0.9 + (cx as f32) * 0.25 + (cy as f32) * 0.17;
                let col = neon_from_phase(phase);
                draw_block(buffer, screen_w, screen_h, x, y, scale, col);
            }
        }
    }

    // Jugador: convertir mundo -> celda -> minimapa
    let (pcx_f, pcy_f) = {
        let ts = map.tile_size() as f32;
        (player.x / ts, player.y / ts)
    };
    let px = margin as f32 + (pcx_f * scale as f32);
    let py = margin as f32 + (pcy_f * scale as f32);

    // Punto del jugador (2x2 px si hay escala pequeña; si scale>=3, usa 3x3)
    let dot = if scale >= 3 { 3 } else { 2 } as usize;
    let px_i = px as isize - (dot as isize / 2);
    let py_i = py as isize - (dot as isize / 2);
    if px_i >= 0 && py_i >= 0 {
        draw_rect(buffer, screen_w, screen_h, px_i as usize, py_i as usize, dot, dot, MM_PLAYER);
    }

    // Flecha/dirección del jugador
    let (dx, dy) = player.dir();
    let line_len = (8 * scale) as f32; // longitud de la flecha en píxeles
    let x2 = (px + dx * line_len) as i32;
    let y2 = (py + dy * line_len) as i32;
    draw_line(buffer, screen_w, screen_h, px as i32, py as i32, x2, y2, MM_PLAYER);

    // Objetivo: dibujar marcador si cae dentro del área visible del minimapa
    let ts2 = map.tile_size() as f32;
    let ocx_f = obj_x / ts2;
    let ocy_f = obj_y / ts2;
    let ocx = ocx_f as usize;
    let ocy = ocy_f as usize;
    if ocx < cells_x && ocy < cells_y {
        let ox = margin + ocx * scale;
        let oy = margin + ocy * scale;
        let ms: usize = if scale >= 3 { 3 } else { 2 };
        let mx = ox.saturating_sub(ms / 2);
        let my = oy.saturating_sub(ms / 2);
        draw_rect(buffer, screen_w, screen_h, mx, my, ms, ms, MM_OBJECTIVE);
    }
}

/// Dibuja toda la escena en el framebuffer.
pub fn draw_scene(buffer: &mut [u32], screen_w: usize, screen_h: usize, map: &Map, player: &Player, obj_x: f32, obj_y: f32, anim_t: f32) {
    assert_eq!(buffer.len(), screen_w * screen_h, "buffer size mismatch");

    // 1) Fondo: cielo (arriba) y suelo (abajo)
    let half = screen_h / 2;
    for y in 0..half {
        let row = y * screen_w;
        buffer[row..row + screen_w].fill(SKY);
    }
    for y in half..screen_h {
        let row = y * screen_w;
        buffer[row..row + screen_w].fill(FLOOR);
    }

    // 2) Ray casting para cada columna
    let hits: Vec<RayHit> = raycaster::cast_all_rays(map, player, screen_w);

    // Proyección: distancia al plano de proyección en píxeles
    let proj_plane = (screen_w as f32 / 2.0) / (player.fov * 0.5).tan();

    for x in 0..screen_w {
        let hit = hits[x];
        if !hit.dist_px.is_finite() || hit.wall_id == 0 { continue; }

        // Altura de la pared en píxeles: proporcional a TILE_SIZE / dist
        let mut col_h = (TILE_SIZE as f32 * proj_plane / hit.dist_px).max(1.0);
        if col_h > screen_h as f32 { col_h = screen_h as f32; }

        let col_h_i = col_h as i32;
        let center = (screen_h / 2) as i32;
        let y1 = (center - col_h_i / 2).max(0);
        let y2 = (center + col_h_i / 2).min(screen_h as i32 - 1);

        // Color base por ID (animado)
        let mut color = wall_color_anim(hit.wall_id, anim_t);
        // Sombreado simple: caras horizontales un poco más oscuras
        if !hit.hit_vertical {
            color = shade(color, 0.75);
        }

        // Dibuja columna
        for yi in y1 as usize..=y2 as usize {
            put_pixel(buffer, screen_w, screen_h, x, yi, color);
        }
    }

    // === OBJETIVO: Cubo “flotante” con oclusión; marcador HUD si no es visible ===
    {
        let ox = obj_x;
        let oy = obj_y;
        let dx = ox - player.x;
        let dy = oy - player.y;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist.is_finite() && dist > 1.0 {
            // Ángulo relativo al jugador en [-PI, PI]
            let mut rel = dy.atan2(dx) - player.angle;
            while rel > PI { rel -= 2.0 * PI; }
            while rel < -PI { rel += 2.0 * PI; }

            let mut drew_any = false;

            // Intento de dibujar si cae dentro del FOV (con pequeño margen)
            if rel.abs() <= player.fov * 0.6 {
                let screen_center = (screen_w as f32) * 0.5;
                let screen_x = screen_center + rel.tan() * proj_plane;

                // Tamaño base en píxeles proporcional a TILE_SIZE/dist
                let base = (TILE_SIZE as f32) * proj_plane / dist;
                let cube = (base * 0.9).max(6.0);       // ancho del cubo
                let front_h = (cube * 0.7).max(3.0);    // alto del frente
                let top_h = (cube * 0.28).max(2.0);     // alto de la tapa
                let half_w = (cube * 0.5).max(2.0);

                let left = (screen_x - half_w).floor() as i32;
                let right = (screen_x + half_w).ceil() as i32;

                let center_y = (screen_h as f32) * 0.5;
                // elevación leve para simular que flota
                let lift = (cube * 0.18) as f32;
                let front_top_f = center_y - front_h * 0.5 - lift;
                let front_bot_f = center_y + front_h * 0.5 - lift;
                let top_top_f = front_top_f - top_h;
                let top_bot_f = front_top_f;

                let front_top = front_top_f.max(0.0) as i32;
                let front_bot = front_bot_f.min((screen_h - 1) as f32) as i32;
                let top_top = top_top_f.max(0.0) as i32;
                let top_bot = top_bot_f.min((screen_h - 1) as f32) as i32;

                let body = OBJ_COLOR;                  // frente
                let top_col = shade(OBJ_COLOR, 0.9);   // tapa ligeramente más oscura
                let edge = 0x000000;                   // bordes

                // Relleno por columnas con test de profundidad por-ray
                for sx in left.max(0)..=right.min(screen_w as i32 - 1) {
                    if dist <= hits[sx as usize].dist_px - 0.5 {
                        // frente
                        for sy in front_top..=front_bot {
                            put_pixel(buffer, screen_w, screen_h, sx as usize, sy as usize, body);
                        }
                        // tapa (sobre el frente)
                        for sy in top_top..=top_bot {
                            put_pixel(buffer, screen_w, screen_h, sx as usize, sy as usize, top_col);
                        }
                        drew_any = true;
                    }
                }

                // Bordes verticales del frente (izq/der), dibujados al final por encima
                let edge_w = 1;
                for sx in left.max(0)..=(left + edge_w).min(screen_w as i32 - 1) {
                    if dist <= hits[sx as usize].dist_px - 0.5 {
                        for sy in front_top..=front_bot {
                            put_pixel(buffer, screen_w, screen_h, sx as usize, sy as usize, edge);
                        }
                        drew_any = true;
                    }
                }
                for sx in (right - edge_w).max(0)..=right.min(screen_w as i32 - 1) {
                    if dist <= hits[sx as usize].dist_px - 0.5 {
                        for sy in front_top..=front_bot {
                            put_pixel(buffer, screen_w, screen_h, sx as usize, sy as usize, edge);
                        }
                        drew_any = true;
                    }
                }

                // Borde superior de la tapa
                for sx in left.max(0)..=right.min(screen_w as i32 - 1) {
                    if dist <= hits[sx as usize].dist_px - 0.5 {
                        let y = top_top;
                        if y >= 0 && y < screen_h as i32 {
                            put_pixel(buffer, screen_w, screen_h, sx as usize, y as usize, edge);
                        }
                        drew_any = true;
                    }
                }

                // Si estaba en FOV pero quedó totalmente ocluido por paredes, dibuja un marcador en el borde superior.
                if !drew_any {
                    let sx = screen_x.round() as i32;
                    let clamped_x = sx.clamp(0, screen_w as i32 - 1);
                    for yy in 10..=22 {
                        put_pixel(buffer, screen_w, screen_h, clamped_x as usize, yy as usize, OBJ_COLOR);
                    }
                    // engrosar 1px a cada lado
                    if clamped_x > 0 {
                        for yy in 12..=20 { put_pixel(buffer, screen_w, screen_h, (clamped_x - 1) as usize, yy as usize, OBJ_COLOR); }
                    }
                    if clamped_x < screen_w as i32 - 1 {
                        for yy in 12..=20 { put_pixel(buffer, screen_w, screen_h, (clamped_x + 1) as usize, yy as usize, OBJ_COLOR); }
                    }
                }
            } else {
                // Fuera de FOV: marcador lateral (izq/der) apuntando hacia la dirección del objetivo
                let screen_center = (screen_w as f32) * 0.5;
                let screen_x = screen_center + rel.tan() * proj_plane;
                let at_left = screen_x < 0.0;
                let x = if at_left { 0 } else { (screen_w as i32 - 1) };
                // flecha vertical simple
                for yy in 10..=26 {
                    put_pixel(buffer, screen_w, screen_h, x as usize, yy as usize, OBJ_COLOR);
                    if at_left && x + 1 < screen_w as i32 { put_pixel(buffer, screen_w, screen_h, (x + 1) as usize, yy as usize, OBJ_COLOR); }
                    if !at_left && x - 1 >= 0 { put_pixel(buffer, screen_w, screen_h, (x - 1) as usize, yy as usize, OBJ_COLOR); }
                }
            }
        }
    }
}

// ====== MENÚ DE BIENVENIDA (un botón: "Jugar") ======
const MENU_BG: u32 = 0x0B0B12;     // negro azulado
const MENU_PANEL: u32 = 0x121433;  // panel azul profundo
const BTN_IDLE: u32 = 0x2837A1;    // azul intenso
const BTN_HILITE: u32 = 0x3D5AFE;  // indigo vibrante
const BTN_BORDER: u32 = 0xB3C3FF;  // borde claro

pub fn menu_button_rects(screen_w: usize, screen_h: usize) -> ((usize, usize, usize, usize), (usize, usize, usize, usize)) {
    let panel_w = (screen_w as f32 * 0.8) as usize;
    let panel_h = (screen_h as f32 * 0.6) as usize;
    let px = (screen_w - panel_w) / 2;
    let py = (screen_h - panel_h) / 2;

    let bw = 220usize; let bh = 60usize;
    let gap = 24usize;
    let total_w = bw * 2 + gap;
    let bx1 = px + (panel_w.saturating_sub(total_w)) / 2;
    let by = py + (panel_h.saturating_sub(bh)) / 2;
    let bx2 = bx1 + bw + gap;

    let r1 = (bx1, by, bw, bh);
    let r2 = (bx2, by, bw, bh);
    (r1, r2)
}

pub fn draw_menu(buffer: &mut [u32], screen_w: usize, screen_h: usize, selected_idx: usize) {
    // Fondo completo
    draw_rect(buffer, screen_w, screen_h, 0, 0, screen_w, screen_h, MENU_BG);

    // Panel central
    let panel_w = (screen_w as f32 * 0.8) as usize;
    let panel_h = (screen_h as f32 * 0.6) as usize;
    let px = (screen_w - panel_w) / 2;
    let py = (screen_h - panel_h) / 2;
    draw_rect(buffer, screen_w, screen_h, px, py, panel_w, panel_h, MENU_PANEL);

    // Título burlón
    draw_text_centered5x7(buffer, screen_w, screen_h, screen_w/2, py + 28, "YOU CLOWN!", 2, TEXT_COLOR);

    // Botones: NORMAL (idx 0) y DIFICIL (idx 1)
    let (r1, r2) = menu_button_rects(screen_w, screen_h);
    let buttons = [r1, r2];
    for (i, &(x, y, w, h)) in buttons.iter().enumerate() {
        let bg = if i == selected_idx { BTN_HILITE } else { BTN_IDLE };
        draw_rect(buffer, screen_w, screen_h, x, y, w, h, bg);
        // Borde
        for xx in x..x + w { put_pixel(buffer, screen_w, screen_h, xx, y, BTN_BORDER); put_pixel(buffer, screen_w, screen_h, xx, y + h - 1, BTN_BORDER); }
        for yy in y..y + h { put_pixel(buffer, screen_w, screen_h, x, yy, BTN_BORDER); put_pixel(buffer, screen_w, screen_h, x + w - 1, yy, BTN_BORDER); }
        // Texto
        let label = if i == 0 { "NORMAL" } else { "DIFICIL" }; // sin acento para la fuente 5x7
        draw_text_centered5x7(buffer, screen_w, screen_h, x + w/2, y + h/2 - 7, label, 2, TEXT_SHADOW);
        draw_text_centered5x7(buffer, screen_w, screen_h, x + w/2, y + h/2 - 8, label, 2, TEXT_COLOR);
    }

    // Hint inferior
    draw_text_centered5x7(buffer, screen_w, screen_h, screen_w/2, py + panel_h - 28, "ENTER O CLIC", 1, TEXT_COLOR);
}

/// Pantalla de victoria simple
pub fn draw_victory(buffer: &mut [u32], screen_w: usize, screen_h: usize) {
    // Fondo
    draw_rect(buffer, screen_w, screen_h, 0, 0, screen_w, screen_h, 0x101010);

    // Panel central
    let panel_w = (screen_w as f32 * 0.7) as usize;
    let panel_h = (screen_h as f32 * 0.4) as usize;
    let px = (screen_w - panel_w) / 2;
    let py = (screen_h - panel_h) / 2;
    draw_rect(buffer, screen_w, screen_h, px, py, panel_w, panel_h, 0x181818);

    draw_text_centered5x7(buffer, screen_w, screen_h, screen_w/2, py + 24, "YOU CLOWN!", 3, 0xEEEEEE);
    draw_text_centered5x7(buffer, screen_w, screen_h, screen_w/2, py + 24 + 1, "YOU CLOWN!", 3, 0xFFFFFF);

    draw_text_centered5x7(buffer, screen_w, screen_h, screen_w/2, py + panel_h/2, "YOU GOT IT", 2, 0xDDDDDD);
    draw_text_centered5x7(buffer, screen_w, screen_h, screen_w/2, py + panel_h - 28, "ENTER O CLIC", 1, 0xBBBBBB);
}


// ====== HUD FPS ======
pub fn draw_fps_hud(buffer: &mut [u32], screen_w: usize, screen_h: usize, fps: u32) {
    let margin = 8usize;
    let text = format!("FPS {}", fps);
    // Sombra
    draw_text5x7(buffer, screen_w, screen_h, margin + 1, margin + 1, &text, 2, TEXT_SHADOW);
    // Texto
    draw_text5x7(buffer, screen_w, screen_h, margin, margin, &text, 2, TEXT_COLOR);
}