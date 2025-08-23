pub struct Map {
    tile_size: u32,
    grid: Vec<Vec<u8>>, // dinámico para facilitar cambios de tamaño
}

// Parámetros del mapa (expandido a 64x64 = 4× área del 32x32 anterior)
pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 64;
pub const TILE_SIZE: u32 = 40; // píxeles por celda (mundo ≈ 2560x2560)

impl Map {
    /// Crea el mapa con el TILE_SIZE definido.
    pub fn new() -> Self {
        Self { tile_size: TILE_SIZE, grid: build_grid() }
    }

    /// Ancho en celdas.
    pub fn width(&self) -> usize { WIDTH }

    /// Alto en celdas.
    pub fn height(&self) -> usize { HEIGHT }

    /// Tamaño de celda en píxeles.
    pub fn tile_size(&self) -> u32 { self.tile_size }

    /// ¿(cx, cy) dentro del rango del grid?
    pub fn in_bounds(&self, cx: i32, cy: i32) -> bool {
        cx >= 0 && cy >= 0 && (cx as usize) < WIDTH && (cy as usize) < HEIGHT
    }

    /// ID crudo de la celda (None si está fuera de rango).
    pub fn cell_id(&self, cx: i32, cy: i32) -> Option<u8> {
        if !self.in_bounds(cx, cy) { return None; }
        Some(self.grid[cy as usize][cx as usize])
    }

    /// ¿La celda es pared? (true si ID > 0). Fuera de rango => false.
    pub fn is_wall(&self, cx: i32, cy: i32) -> bool {
        self.cell_id(cx, cy).map(|id| id > 0).unwrap_or(false)
    }

    /// Convierte coordenadas de mundo (píxeles) a celda (enteros).
    pub fn world_to_cell(&self, x: f32, y: f32) -> (i32, i32) {
        let ts = self.tile_size as f32;
        let cx = (x / ts).floor() as i32;
        let cy = (y / ts).floor() as i32;
        (cx, cy)
    }

    /// Centro de una celda (en coordenadas de mundo/píxeles).
    pub fn cell_center_world(&self, cx: i32, cy: i32) -> Option<(f32, f32)> {
        if !self.in_bounds(cx, cy) { return None; }
        let ts = self.tile_size as f32;
        let x = (cx as f32 + 0.5) * ts;
        let y = (cy as f32 + 0.5) * ts;
        Some((x, y))
    }

    /// Punto de spawn recomendado (en mundo/píxeles), dentro de un espacio libre.
    pub fn recommended_spawn(&self) -> (f32, f32) {
        // Cerca de (1,1) sigue siendo libre en este layout.
        self.cell_center_world(1, 1).unwrap()
    }
}

/// Construye un grid 64x64 con muros perimetrales (ID=1) y muros internos (ID=2)
fn build_grid() -> Vec<Vec<u8>> {
    let mut g = vec![vec![0u8; WIDTH]; HEIGHT];

    // Borde perimetral ID=1
    for x in 0..WIDTH {
        g[0][x] = 1;
        g[HEIGHT - 1][x] = 1;
    }
    for y in 0..HEIGHT {
        g[y][0] = 1;
        g[y][WIDTH - 1] = 1;
    }

    // Muros verticales interiores (ID=2) cada 8 columnas, con aperturas periódicas
    for vx in (6..WIDTH - 6).step_by(8) {
        for y in 2..HEIGHT - 2 {
            if y % 10 == 5 { continue; } // abrir "puertas"
            g[y][vx] = 2;
        }
    }

    // Muros horizontales interiores (ID=2) cada 8 filas, con aperturas periódicas
    for hy in (6..HEIGHT - 6).step_by(8) {
        for x in 2..WIDTH - 2 {
            if x % 12 == 6 { continue; } // abrir "puertas"
            g[hy][x] = 2;
        }
    }

    g
}