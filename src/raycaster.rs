//! Ray casting (DDA) para el MVP.
//! Devuelve, por columna de pantalla, la distancia perpendicular al primer muro.

use crate::map::{Map, TILE_SIZE, WIDTH as MAP_W, HEIGHT as MAP_H};
use crate::player::Player;

#[derive(Clone, Copy, Debug, Default)]
pub struct RayHit {
    /// Distancia perpendicular al muro en **píxeles** (coords del mundo).
    pub dist_px: f32,
    /// ID de pared (0 si no se encontró; en mapa cerrado siempre > 0).
    pub wall_id: u8,
    /// true si el cruce fue con borde vertical (eje X), false si horizontal (eje Y).
    pub hit_vertical: bool,
}

/// Lanza todos los rayos necesarios para el ancho de la pantalla.
pub fn cast_all_rays(map: &Map, player: &Player, screen_w: usize) -> Vec<RayHit> {
    let mut hits = vec![RayHit::default(); screen_w];
    for col in 0..screen_w {
        hits[col] = cast_ray_for_column(map, player, screen_w, col);
    }
    hits
}

fn cast_ray_for_column(map: &Map, player: &Player, screen_w: usize, col: usize) -> RayHit {
    // Ángulo del rayo dentro del FOV
    let t = if screen_w > 1 { col as f32 / (screen_w as f32 - 1.0) } else { 0.5 };
    let ray_angle = player.angle - player.fov * 0.5 + t * player.fov;
    let ray_dir_x = ray_angle.cos();
    let ray_dir_y = ray_angle.sin();

    // Posición del jugador en **unidades de celda**
    let pos_cell_x = player.x / TILE_SIZE as f32;
    let pos_cell_y = player.y / TILE_SIZE as f32;

    // Celda actual
    let mut map_x = pos_cell_x.floor() as i32;
    let mut map_y = pos_cell_y.floor() as i32;

    // Evitar divisiones por cero
    let inv_dx = if ray_dir_x.abs() < 1e-6 { f32::INFINITY } else { 1.0 / ray_dir_x };
    let inv_dy = if ray_dir_y.abs() < 1e-6 { f32::INFINITY } else { 1.0 / ray_dir_y };

    let delta_dist_x = inv_dx.abs();
    let delta_dist_y = inv_dy.abs();

    // Preparar longitud desde el borde de celda hasta el primer corte en X/Y
    let (step_x, mut side_dist_x) = if ray_dir_x < 0.0 {
        let dist = (pos_cell_x - map_x as f32) * delta_dist_x;
        (-1, dist)
    } else {
        let dist = ((map_x as f32 + 1.0) - pos_cell_x) * delta_dist_x;
        (1, dist)
    };

    let (step_y, mut side_dist_y) = if ray_dir_y < 0.0 {
        let dist = (pos_cell_y - map_y as f32) * delta_dist_y;
        (-1, dist)
    } else {
        let dist = ((map_y as f32 + 1.0) - pos_cell_y) * delta_dist_y;
        (1, dist)
    };

    // DDA loop
    let mut hit_id: u8 = 0;
    let mut hit_vertical = false;

    // Límite de pasos de seguridad (mapa cerrado debe chocar antes)
    let max_steps = (MAP_W.max(MAP_H) * 4) as usize;
    for _ in 0..max_steps {
        if side_dist_x < side_dist_y {
            side_dist_x += delta_dist_x;
            map_x += step_x;
            hit_vertical = true; // cruzamos un borde vertical
        } else {
            side_dist_y += delta_dist_y;
            map_y += step_y;
            hit_vertical = false; // cruzamos un borde horizontal
        }

        if !map.in_bounds(map_x, map_y) {
            return RayHit::default();
        }
        if let Some(id) = map.cell_id(map_x, map_y) {
            if id > 0 { hit_id = id; break; }
        }
    }

    if hit_id == 0 { return RayHit::default(); }

    // Distancia perpendicular en **unidades de celda**
    let perp_cells = if hit_vertical {
        // Cruce en X
        let denom = if ray_dir_x.abs() < 1e-6 { 1e-6 } else { ray_dir_x };
        ((map_x as f32 - pos_cell_x) + (1.0 - step_x as f32) * 0.5) / denom
    } else {
        // Cruce en Y
        let denom = if ray_dir_y.abs() < 1e-6 { 1e-6 } else { ray_dir_y };
        ((map_y as f32 - pos_cell_y) + (1.0 - step_y as f32) * 0.5) / denom
    };

    let dist_px = perp_cells.abs() * TILE_SIZE as f32;

    RayHit { dist_px, wall_id: hit_id, hit_vertical }
}
