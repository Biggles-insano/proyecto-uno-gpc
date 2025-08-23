//! Estado y movimiento del jugador (MVP, con y sin colisión).
//! Mantiene posición en mundo (píxeles) y ángulo de vista (radianes).
//! Provee utilidades de dirección y métodos de movimiento básicos.

use core::f32::consts::{PI, FRAC_PI_3};
use crate::map::Map;

/// Radio de colisión del jugador en píxeles (≈ 0.3 * TILE_SIZE si TILE=40 → ~12px)
pub const RADIUS_PX: f32 = 12.0;
/// Margen pequeño para evitar vibraciones en bordes
pub const EPSILON_PX: f32 = 0.75;

/// Representa al jugador en el mundo.
pub struct Player {
    pub x: f32,       // posición X en mundo (px)
    pub y: f32,       // posición Y en mundo (px)
    pub angle: f32,   // orientación en radianes (0 mira a +X)
    pub move_speed: f32, // px/seg
    pub rot_speed: f32,  // rad/seg
    pub fov: f32,        // campo de visión en radianes (ej. ~60°)
}

impl Player {
    /// Crea un jugador en (x, y). Ángulo inicial mirando hacia +X (0 rad).
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            angle: 0.0,
            move_speed: 160.0, // ajustable
            rot_speed: 2.6,    // ajustable (~150°/s)
            fov: FRAC_PI_3,    // ~60°
        }
    }

    /// Crea un jugador usando un punto de spawn recomendado del mapa.
    pub fn from_map_spawn(map: &crate::map::Map) -> Self {
        let (sx, sy) = map.recommended_spawn();
        Self::new(sx, sy)
    }

    /// Vector dirección normalizado (cos(angle), sin(angle)).
    pub fn dir(&self) -> (f32, f32) {
        (self.angle.cos(), self.angle.sin())
    }

    /// Vector a la derecha (perpendicular) normalizado.
    pub fn right(&self) -> (f32, f32) {
        let (dx, dy) = self.dir();
        (-dy, dx)
    }

    /// Girar izquierda.
    pub fn turn_left(&mut self, dt: f32) {
        self.angle -= self.rot_speed * dt;
        self.normalize_angle();
    }

    /// Girar derecha.
    pub fn turn_right(&mut self, dt: f32) {
        self.angle += self.rot_speed * dt;
        self.normalize_angle();
    }

    /// =======================
    /// Movimiento SIN colisión
    /// =======================

    /// Avanzar hacia adelante (sin colisión).
    pub fn forward(&mut self, dt: f32) {
        let (dx, dy) = self.dir();
        self.x += dx * self.move_speed * dt;
        self.y += dy * self.move_speed * dt;
    }

    /// Retroceder (sin colisión).
    pub fn backward(&mut self, dt: f32) {
        let (dx, dy) = self.dir();
        self.x -= dx * self.move_speed * dt;
        self.y -= dy * self.move_speed * dt;
    }

    /// Desplazamiento lateral izquierdo (strafe) sin colisión.
    pub fn strafe_left(&mut self, dt: f32) {
        let (rx, ry) = self.right();
        self.x -= rx * self.move_speed * dt;
        self.y -= ry * self.move_speed * dt;
    }

    /// Desplazamiento lateral derecho (strafe) sin colisión.
    pub fn strafe_right(&mut self, dt: f32) {
        let (rx, ry) = self.right();
        self.x += rx * self.move_speed * dt;
        self.y += ry * self.move_speed * dt;
    }

    /// =======================
    /// Movimiento CON colisión
    /// =======================

    /// Intenta mover aplicando colisión (resolución por ejes X luego Y).
    pub fn try_move(&mut self, dx: f32, dy: f32, map: &Map) {
        // Mover en X
        if dx != 0.0 {
            let nx = self.x + dx;
            if !self.collides_at(nx, self.y, map) {
                self.x = nx;
            }
        }
        // Mover en Y
        if dy != 0.0 {
            let ny = self.y + dy;
            if !self.collides_at(self.x, ny, map) {
                self.y = ny;
            }
        }
    }

    /// Avanzar con colisión.
    pub fn forward_collide(&mut self, dt: f32, map: &Map) {
        let (dx, dy) = self.dir();
        self.try_move(dx * self.move_speed * dt, dy * self.move_speed * dt, map);
    }

    /// Retroceder con colisión.
    pub fn backward_collide(&mut self, dt: f32, map: &Map) {
        let (dx, dy) = self.dir();
        self.try_move(-dx * self.move_speed * dt, -dy * self.move_speed * dt, map);
    }

    /// Strafe izquierda con colisión.
    pub fn strafe_left_collide(&mut self, dt: f32, map: &Map) {
        let (rx, ry) = self.right();
        self.try_move(-rx * self.move_speed * dt, -ry * self.move_speed * dt, map);
    }

    /// Strafe derecha con colisión.
    pub fn strafe_right_collide(&mut self, dt: f32, map: &Map) {
        let (rx, ry) = self.right();
        self.try_move(rx * self.move_speed * dt, ry * self.move_speed * dt, map);
    }

    /// Devuelve true si la posición (wx, wy) con el radio del jugador colisiona con una pared.
    fn collides_at(&self, wx: f32, wy: f32, map: &Map) -> bool {
        let r = RADIUS_PX + EPSILON_PX;
        // Muestra 4 puntos cardinales del círculo
        let samples = [
            (wx - r, wy), // izquierda
            (wx + r, wy), // derecha
            (wx, wy - r), // arriba
            (wx, wy + r), // abajo
        ];
        for (px, py) in samples.iter() {
            let (cx, cy) = map.world_to_cell(*px, *py);
            if !map.in_bounds(cx, cy) { return true; } // fuera = pared
            if map.is_wall(cx, cy) { return true; }
        }
        false
    }

    /// Normaliza el ángulo a [-PI, PI).
    fn normalize_angle(&mut self) {
        let mut a = self.angle;
        // Llevar a rango [-PI, PI)
        while a >= PI { a -= 2.0 * PI; }
        while a < -PI { a += 2.0 * PI; }
        self.angle = a;
    }
}