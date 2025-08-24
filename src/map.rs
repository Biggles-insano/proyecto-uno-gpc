pub struct Map {
    tile_size: u32,
    grid: Vec<Vec<u8>>, // 0 = libre, >0 = pared (ID)
    seed: u32,
}

pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 64;
pub const TILE_SIZE: u32 = 40;

impl Map {
    /// Variante por defecto (seed = 0)
    pub fn new() -> Self { Self::new_with_seed(0) }

    /// Crea un mapa variando la semilla. Mapas con semillas distintas generan laberintos distintos.
    pub fn new_with_seed(seed: u32) -> Self {
        Self { tile_size: TILE_SIZE, grid: build_grid(seed), seed }
    }

    pub fn width(&self) -> usize { WIDTH }
    pub fn height(&self) -> usize { HEIGHT }
    pub fn tile_size(&self) -> u32 { self.tile_size }
    pub fn seed(&self) -> u32 { self.seed }

    pub fn in_bounds(&self, cx: i32, cy: i32) -> bool {
        cx >= 0 && cy >= 0 && (cx as usize) < WIDTH && (cy as usize) < HEIGHT
    }

    pub fn cell_id(&self, cx: i32, cy: i32) -> Option<u8> {
        if !self.in_bounds(cx, cy) { return None; }
        Some(self.grid[cy as usize][cx as usize])
    }

    pub fn is_wall(&self, cx: i32, cy: i32) -> bool {
        self.cell_id(cx, cy).map(|id| id > 0).unwrap_or(false)
    }

    pub fn world_to_cell(&self, x: f32, y: f32) -> (i32, i32) {
        let ts = self.tile_size as f32;
        let cx = (x / ts).floor() as i32;
        let cy = (y / ts).floor() as i32;
        (cx, cy)
    }

    pub fn cell_center_world(&self, cx: i32, cy: i32) -> Option<(f32, f32)> {
        if !self.in_bounds(cx, cy) { return None; }
        let ts = self.tile_size as f32;
        let x = (cx as f32 + 0.5) * ts;
        let y = (cy as f32 + 0.5) * ts;
        Some((x, y))
    }

    /// Punto de spawn recomendado, esquina NW del laberinto (celda libre 1,1)
    pub fn recommended_spawn(&self) -> (f32, f32) {
        self.cell_center_world(1, 1).unwrap()
    }

    /// ¿La celda es libre (pasillo)?
    pub fn is_free(&self, cx: i32, cy: i32) -> bool {
        matches!(self.cell_id(cx, cy), Some(0))
    }

    /// Devuelve la celda objetivo (determinística por seed), lejos del spawn.
    /// Elige una celda libre maximizando distancia al spawn con un pequeño jitter por hash.
    pub fn objective_cell(&self) -> (i32, i32) {
        let (sx, sy) = (1i32, 1i32); // spawn en celda (1,1)
        let mut best = (sx, sy);
        let mut best_score: i64 = i64::MIN;
        for y in 1..(HEIGHT as i32 - 1) {
            for x in 1..(WIDTH as i32 - 1) {
                if !self.is_free(x, y) { continue; }
                let dx = x - sx; let dy = y - sy;
                let d2 = (dx as i64 * dx as i64) + (dy as i64 * dy as i64);
                // hash determinista con seed para desempatar
                let mut h = self.seed
                    ^ (x as u32).wrapping_mul(73856093)
                    ^ (y as u32).wrapping_mul(19349663)
                    ^ 0x9E3779B9;
                h ^= h << 13; h ^= h >> 17; h ^= h << 5;
                let jitter = (h & 0xFF) as i64; // 0..255
                let score = d2 * 256 + jitter;
                if score > best_score { best_score = score; best = (x, y); }
            }
        }
        best
    }

    /// Centro del objetivo en coordenadas de mundo (píxeles).
    pub fn objective_world(&self) -> (f32, f32) {
        let (cx, cy) = self.objective_cell();
        self.cell_center_world(cx, cy).unwrap()
    }
}

/// Genera un laberinto perfecto con ampliación selectiva de pasillos y pilares decorativos.
/// - Perímetro: ID=1 (muro firme)
/// - Muros internos: ID=2
/// - Pasillos: 0
/// - Pilares decorativos: ID=3
fn build_grid(seed: u32) -> Vec<Vec<u8>> {
    // Base: todo muro interno (2) y perímetro (1)
    let mut g = vec![vec![2u8; WIDTH]; HEIGHT];
    for x in 0..WIDTH { g[0][x] = 1; g[HEIGHT - 1][x] = 1; }
    for y in 0..HEIGHT { g[y][0] = 1; g[y][WIDTH - 1] = 1; }

    // Malla de celdas impares, inicio (1,1)
    let (sx, sy) = (1usize, 1usize);
    g[sy][sx] = 0;

    let mut stack: Vec<(usize, usize)> = Vec::with_capacity((WIDTH * HEIGHT) / 4);
    stack.push((sx, sy));

    // Saltos de 2 celdas (E, O, S, N)
    const DIRS: [(i32, i32); 4] = [(2, 0), (-2, 0), (0, 2), (0, -2)];

    // DFS con barajado determinista influido por la semilla
    let mut order = [0usize, 1, 2, 3];
    while let Some(&(cx, cy)) = stack.last() {
        // xorshift32 mezclando (cx,cy) y seed
        let mut s = seed
            ^ (cx as u32).wrapping_mul(0x9E3779B1)
            ^ (cy as u32).wrapping_mul(0x85EBCA77)
            ^ 0x27D4EB2D;
        order = [0, 1, 2, 3];
        for i in (1..4).rev() {
            s ^= s << 13; s ^= s >> 17; s ^= s << 5;
            let j = (s as usize) % (i + 1);
            let tmp = order[i]; order[i] = order[j]; order[j] = tmp;
        }

        let mut advanced = false;
        for &oi in &order {
            let (dx, dy) = DIRS[oi];
            let nx = cx as i32 + dx; let ny = cy as i32 + dy;
            if nx <= 0 || ny <= 0 || nx >= (WIDTH as i32 - 1) || ny >= (HEIGHT as i32 - 1) { continue; }
            let nxu = nx as usize; let nyu = ny as usize;
            if g[nyu][nxu] != 0 {
                let wx = (cx as i32 + dx / 2) as usize;
                let wy = (cy as i32 + dy / 2) as usize;
                g[wy][wx] = 0; // abre muro intermedio
                g[nyu][nxu] = 0; // abre celda destino
                stack.push((nxu, nyu));
                advanced = true;
                break;
            }
        }
        if !advanced { stack.pop(); }
    }

    // Ensanchar pasillos con criterio (solo a lo ancho del segmento)
    {
        let mut to_open: Vec<(usize, usize)> = Vec::new();
        for y in 1..HEIGHT-1 {
            for x in 1..WIDTH-1 {
                if g[y][x] != 0 { continue; }
                let left  = g[y][x.saturating_sub(1)] == 0;
                let right = g[y][x + 1] == 0;
                let up    = g[y.saturating_sub(1)][x] == 0;
                let down  = g[y + 1][x] == 0;

                // Segmento horizontal puro (paredes arriba/abajo)
                if (left || right) && !(up || down) {
                    if y > 1 && g[y - 1][x] == 2 && ((y as u32 + seed) % 2 == 0) { to_open.push((x, y - 1)); }
                    else if y < HEIGHT - 2 && g[y + 1][x] == 2 { to_open.push((x, y + 1)); }
                }
                // Segmento vertical puro (paredes izquierda/derecha)
                else if (up || down) && !(left || right) {
                    if x > 1 && g[y][x - 1] == 2 && ((x as u32 + seed) % 2 == 0) { to_open.push((x - 1, y)); }
                    else if x < WIDTH - 2 && g[y][x + 1] == 2 { to_open.push((x + 1, y)); }
                }
            }
        }
        for (x, y) in to_open { g[y][x] = 0; }
    }

    // Pilares decorativos (ID=3) en áreas abiertas; densidad controlada por seed
    {
        let mut add: Vec<(usize, usize)> = Vec::new();
        for y in 2..HEIGHT - 2 {
            for x in 2..WIDTH - 2 {
                if g[y][x] != 0 { continue; }
                let mut free = 0;
                if g[y - 1][x] == 0 { free += 1; }
                if g[y + 1][x] == 0 { free += 1; }
                if g[y][x - 1] == 0 { free += 1; }
                if g[y][x + 1] == 0 { free += 1; }
                if free >= 3 {
                    // Hash determinista + seed; densidad ≈ 1/12
                    let mut h = seed
                        ^ (x as u32).wrapping_mul(73856093)
                        ^ (y as u32).wrapping_mul(19349663);
                    h ^= h << 13; h ^= h >> 17; h ^= h << 5;
                    if (h % 12) == 0 {
                        if g[y - 1][x] != 3 && g[y + 1][x] != 3 && g[y][x - 1] != 3 && g[y][x + 1] != 3 {
                            add.push((x, y));
                        }
                    }
                }
            }
        }
        for (x, y) in add { g[y][x] = 3; }
    }

    g
}