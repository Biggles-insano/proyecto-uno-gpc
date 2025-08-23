//! Renderizado básico (MVP): cielo, suelo y columnas de paredes.
//! Colores planos por ID de pared y sombreado simple según la cara impactada.

use crate::map::{Map, TILE_SIZE};
use crate::player::Player;
use crate::raycaster::{self, RayHit};

const SKY: u32 = 0x87CEEB;   // azul cielo (0x00RRGGBB)
const FLOOR: u32 = 0x303030; // gris oscuro

// Colores por ID de pared (ajustables luego)
fn wall_color(id: u8) -> u32 {
    match id {
        1 => 0xC0C0C0, // gris claro
        2 => 0xB5651D, // café
        3 => 0x228B22, // verde bosque
        4 => 0x8B0000, // rojo oscuro
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

#[inline]
fn put_pixel(buffer: &mut [u32], w: usize, h: usize, x: usize, y: usize, color: u32) {
    if x < w && y < h {
        buffer[y * w + x] = color;
    }
}

/// Dibuja toda la escena en el framebuffer.
pub fn draw_scene(buffer: &mut [u32], screen_w: usize, screen_h: usize, map: &Map, player: &Player) {
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

        // Color base por ID
        let mut color = wall_color(hit.wall_id);
        // Sombreado simple: caras horizontales un poco más oscuras
        if !hit.hit_vertical {
            color = shade(color, 0.75);
        }

        // Dibuja columna
        for yi in y1 as usize..=y2 as usize {
            put_pixel(buffer, screen_w, screen_h, x, yi, color);
        }
    }
}
