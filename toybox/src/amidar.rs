use super::graphics::{Color, Drawable};
use super::Input;
use failure::Error;
use std::collections::{HashSet, VecDeque};

// Window constants:
pub mod screen {
    pub const GAME_SIZE: (i32, i32) = (160, 250);
    pub const BOARD_OFFSET: (i32, i32) = (16, 37);
    pub const PLAYER_SIZE: (i32, i32) = (7, 7);
    pub const ENEMY_SIZE: (i32, i32) = (7, 7);
    pub const TILE_SIZE: (i32, i32) = (4, 5);
}

mod world {
    use super::screen;
    pub const SCALE: i32 = 16;
    pub const TILE_SIZE: (i32, i32) = (screen::TILE_SIZE.0 * SCALE, screen::TILE_SIZE.1 * SCALE);
}
pub const AMIDAR_BOARD: &str = include_str!("resources/amidar_default_board");
pub const AMIDAR_ENEMY_POSITIONS_DATA: &str = include_str!("resources/amidar_enemy_positions");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}
impl Direction {
    fn delta(&self) -> (i32, i32) {
        match self {
            Direction::Up => (0, -1),
            Direction::Down => (0, 1),
            Direction::Left => (-1, 0),
            Direction::Right => (1, 0),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScreenPoint {
    pub sx: i32,
    pub sy: i32,
}
impl ScreenPoint {
    fn new(sx: i32, sy: i32) -> ScreenPoint {
        ScreenPoint { sx, sy }
    }
    pub fn pixels(&self) -> (i32, i32) {
        (self.sx, self.sy)
    }
}

/// Strongly-typed vector for "world" positioning in Amidar.
#[derive(Debug, Clone)]
pub struct WorldPoint {
    pub x: i32,
    pub y: i32,
}
impl WorldPoint {
    fn new(x: i32, y: i32) -> WorldPoint {
        WorldPoint { x, y }
    }
    pub fn to_screen(&self) -> ScreenPoint {
        ScreenPoint::new(self.x / world::SCALE, self.y / world::SCALE)
    }
    pub fn to_tile(&self) -> TilePoint {
        let mut tx = self.x / world::TILE_SIZE.0;
        let mut ty = self.y / world::TILE_SIZE.1;
        if self.x < 0 {
            tx -= 1;
        }
        if self.y < 0 {
            ty -= 1;
        }
        TilePoint::new(tx, ty)
    }
    pub fn translate(&self, dx: i32, dy: i32) -> WorldPoint {
        WorldPoint::new(self.x + dx, self.y + dy)
    }
}

/// Strongly-typed vector for "tile" positioning in Amidar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TilePoint {
    pub tx: i32,
    pub ty: i32,
}
impl TilePoint {
    pub fn new(tx: i32, ty: i32) -> TilePoint {
        TilePoint { tx, ty }
    }
    pub fn to_world(&self) -> WorldPoint {
        WorldPoint::new(self.tx * world::TILE_SIZE.0, self.ty * world::TILE_SIZE.1)
    }
    pub fn translate(&self, dx: i32, dy: i32) -> TilePoint {
        TilePoint::new(self.tx + dx, self.ty + dy)
    }
    pub fn step(&self, dir: Direction) -> TilePoint {
        let (dx, dy) = dir.delta();
        self.translate(dx, dy)
    }
}

#[derive(Clone, Debug)]
pub struct GridBox {
    pub top_left: TilePoint,
    pub bottom_right: TilePoint,
    pub painted: bool,
}

impl GridBox {
    fn new(top_left: TilePoint, bottom_right: TilePoint) -> GridBox {
        GridBox {
            top_left,
            bottom_right,
            painted: false,
        }
    }
    fn matches(&self, tile: &TilePoint) -> bool {
        let x1 = self.top_left.tx;
        let x2 = self.bottom_right.tx;
        let y1 = self.top_left.ty;
        let y2 = self.bottom_right.ty;

        let xq = tile.tx;
        let yq = tile.ty;

        (x1 <= xq) && (xq <= x2) && (y1 <= yq) && (yq <= y2)
    }
    /// Check whether this box's painting should be updated.
    /// Returns true iff something should change.
    fn should_update_paint(&self, board: &Board) -> bool {
        if self.painted {
            return false;
        }

        let x1 = self.top_left.tx;
        let x2 = self.bottom_right.tx;
        let y1 = self.top_left.ty;
        let y2 = self.bottom_right.ty;

        let top_and_bottom = (x1..=x2).all(|xi| {
            board.is_painted(&TilePoint::new(xi, y1)) && board.is_painted(&TilePoint::new(xi, y2))
        });
        let left_and_right = (y1..=y2).all(|yi| {
            board.is_painted(&TilePoint::new(x1, yi)) && board.is_painted(&TilePoint::new(x2, yi))
        });

        if top_and_bottom && left_and_right {
            return true;
        }
        false
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tile {
    Empty,
    Unpainted,
    Painted,
}
impl Tile {
    fn new_from_char(c: char) -> Result<Tile, Error> {
        match c {
            '=' => Ok(Tile::Unpainted),
            'p' => Ok(Tile::Painted),
            ' ' => Ok(Tile::Empty),
            _ => Err(format_err!("Cannot construct AmidarTile from '{}'", c)),
        }
    }
    fn walkable(&self) -> bool {
        match self {
            Tile::Empty => false,
            Tile::Painted | Tile::Unpainted => true,
        }
    }
}

#[derive(PartialEq)]
pub enum MovementAI {
    Player,
    EnemyLookupAI { next: u32, path: Vec<u32> },
}

impl MovementAI {
    fn reset(&mut self) {
        match self {
            MovementAI::Player => {}
            MovementAI::EnemyLookupAI { next, path } => {
                *next = 0;
            }
        }
    }
    fn choose_next_tile(
        &mut self,
        position: &TilePoint,
        buttons: &[Input],
        board: &Board,
    ) -> Option<TilePoint> {
        match self {
            MovementAI::Player => {
                let left = buttons.contains(&Input::Left);
                let right = buttons.contains(&Input::Right);
                let up = buttons.contains(&Input::Up);
                let down = buttons.contains(&Input::Down);

                let mut input: Option<Direction> = None;
                if left {
                    input = Some(Direction::Left);
                } else if right {
                    input = Some(Direction::Right);
                } else if up {
                    input = Some(Direction::Up);
                } else if down {
                    input = Some(Direction::Down);
                }

                input.and_then(|dir| {
                    let target_tile = position.step(dir);
                    if board.get_tile(&target_tile).walkable() {
                        Some(target_tile)
                    } else {
                        None
                    }
                })
            }
            MovementAI::EnemyLookupAI { next, path } => {
                *next = (*next + 1) % (path.len() as u32);
                Some(board.lookup_position(path[*next as usize]))
            }
        }
    }
}

/// Mob is a videogame slang for "mobile" unit. Players and Enemies are the same struct.
pub struct Mob {
    pub ai: MovementAI,
    pub position: WorldPoint,
    speed: i32,
    step: Option<TilePoint>,
    history: VecDeque<u32>,
}
impl Mob {
    fn new(ai: MovementAI, position: WorldPoint) -> Mob {
        Mob {
            ai,
            position,
            step: None,
            speed: 8,
            history: VecDeque::new(),
        }
    }
    pub fn new_player(position: WorldPoint) -> Mob {
        Mob {
            ai: MovementAI::Player,
            position,
            step: None,
            speed: 8,
            history: VecDeque::new(),
        }
    }
    fn is_player(&self) -> bool {
        self.ai == MovementAI::Player
    }
    fn reset(&mut self, player_start: &TilePoint, board: &Board) {
        self.step = None;
        self.ai.reset();
        self.position = match self.ai {
            MovementAI::Player => player_start.to_world(),
            MovementAI::EnemyLookupAI { ref path, .. } => board.lookup_position(path[0]).to_world(),
        };
        self.history.clear();
    }
    pub fn update(&mut self, buttons: &[Input], board: &mut Board) -> Option<ScoreUpdate> {
        if self.history.is_empty() {
            if let Some(pt) = board.get_junction_id(&self.position.to_tile()) {
                self.history.push_front(pt);
            }
        }

        // Animate/step player movement.
        let next_target = if let Some(ref target) = self.step {
            // Move player 1 step toward its target:
            let world_target = target.to_world();
            let dx = world_target.x - self.position.x;
            let dy = world_target.y - self.position.y;

            if dx == 0 && dy == 0 {
                // We have reached this target tile:
                if let Some(pt) = board.get_junction_id(target) {
                    self.history.push_front(pt);
                }
                None
            } else {
                self.position.x += self.speed * dx.signum();
                self.position.y += self.speed * dy.signum();
                Some(target.clone())
            }
        } else {
            None
        };

        // Rust prevents us from modifying step back to None when we reach it in the above block, since we have bound a reference to the inside of the if-let-Some.
        // We therefore unconditionally return the target from that expression and write it mutably here, where it is obvious it is safe to the rust compiler.
        self.step = next_target;

        // Not an else if -- if a player or enemy reaches a tile they can immediately choose a new target.
        if self.step.is_none() {
            self.step = self
                .ai
                .choose_next_tile(&self.position.to_tile(), buttons, board)
        }

        // Manage history:
        if self.is_player() {
            board.check_paint(&mut self.history).into_option()
        } else {
            if self.history.len() > 12 {
                let _ = self.history.pop_back();
            }
            None
        }
    }
}

lazy_static! {
    static ref DEFAULT_BOARD: Board = Board::try_new().unwrap();
}

#[derive(Clone)]
pub struct Board {
    pub tiles: Vec<Vec<Tile>>,
    pub width: u32,
    pub height: u32,
    pub junctions: HashSet<u32>,
    pub boxes: Vec<GridBox>,
}

pub struct ScoreUpdate {
    pub vertical: i32,
    pub horizontal: i32,
    pub num_boxes: i32,
}
impl ScoreUpdate {
    fn new() -> ScoreUpdate {
        ScoreUpdate {
            vertical: 0,
            horizontal: 0,
            num_boxes: 0,
        }
    }
    fn happened(&self) -> bool {
        self.vertical != 0 || self.horizontal != 0 || self.num_boxes != 0
    }
    fn into_option(self) -> Option<Self> {
        if self.happened() {
            Some(self)
        } else {
            None
        }
    }
}

impl Board {
    pub fn fast_new() -> Board {
        DEFAULT_BOARD.clone()
    }
    fn try_new() -> Result<Board, Error> {
        let mut tiles = Vec::new();
        for line in AMIDAR_BOARD.lines() {
            // Rust will aggregate errors in collect for us if we give it a type-hint.
            let row: Result<Vec<_>, _> = line.chars().map(Tile::new_from_char).collect();
            // Exit function if row is errorful.
            tiles.push(row?);
        }
        let width = tiles[0].len() as u32;
        let height = tiles.len() as u32;

        let mut board = Board {
            tiles,
            width,
            height,
            junctions: HashSet::new(),
            boxes: Vec::new(),
        };
        board.init_junctions();
        debug_assert!(board.boxes.is_empty());
        board.boxes = board
            .junctions
            .iter()
            .flat_map(|pt| board.junction_corners(*pt))
            .collect();
        Ok(board)
    }

    fn is_corner(&self, tx: i32, ty: i32) -> bool {
        let last_y = (self.height as i32) - 1;
        let last_x = (self.width as i32) - 1;
        (tx == 0 || tx == last_x) && (ty == 0 || ty == last_y)
    }

    fn init_junctions(&mut self) {
        // Only run this function once.
        debug_assert!(self.junctions.is_empty());

        for (y, row) in self.tiles.iter().enumerate() {
            let y = y as i32;
            for (x, cell) in row.iter().enumerate() {
                let x = x as i32;
                if cell.walkable() {
                    let neighbors = [(x + 1, y), (x, y + 1), (x - 1, y), (x, y - 1)];
                    let walkable_neighbors = neighbors
                        .iter()
                        .filter(|(nx, ny)| self.get_tile(&TilePoint::new(*nx, *ny)).walkable())
                        .count();
                    if walkable_neighbors > 2 || self.is_corner(x, y) {
                        let y = y as u32;
                        let x = x as u32;
                        let _ = self.junctions.insert(y * self.width + x);
                    }
                }
            }
        }
    }

    fn is_painted(&self, xy: &TilePoint) -> bool {
        self.get_tile(xy) == Tile::Painted
    }

    /// Find the junction in direction ``search`` starting from ``source`` that allows us to walk in ``walkable`` direction.
    fn junction_neighbor(
        &self,
        source: u32,
        search: Direction,
        walkable: Direction,
    ) -> Option<u32> {
        let mut pos = self.lookup_position(source);
        loop {
            pos = pos.step(search);
            let num = self.tile_id(&pos)?;
            if self.junctions.contains(&num) && self.get_tile(&pos.step(walkable)).walkable() {
                return Some(num);
            }
        }
        unreachable!()
    }

    fn junction_corners(&self, source: u32) -> Option<GridBox> {
        let mut src = source;
        // Find the first junction to the right that lets us go down.
        let right =
            self.lookup_position(self.junction_neighbor(src, Direction::Right, Direction::Down)?);
        // Find the first junction down that lets us go right.
        let down =
            self.lookup_position(self.junction_neighbor(src, Direction::Down, Direction::Right)?);
        // There needs to be a bottom_right junction that connects this box.
        let down_right = self.tile_id(&TilePoint::new(right.tx, down.ty))?;
        if self.junctions.contains(&down_right) {
            Some(GridBox::new(
                self.lookup_position(source),
                self.lookup_position(down_right),
            ))
        } else {
            None
        }
    }

    fn tile_id(&self, tile: &TilePoint) -> Option<u32> {
        if tile.ty < 0
            || tile.tx < 0
            || tile.ty >= self.height as i32
            || tile.tx >= self.width as i32
        {
            return None;
        }
        let y = tile.ty as u32;
        let x = tile.tx as u32;
        Some(y * self.width + x)
    }

    fn get_junction_id(&self, tile: &TilePoint) -> Option<u32> {
        self.tile_id(tile)
            .filter(|num| self.junctions.contains(num))
    }

    /// Check whether the painting of segment t1 .. t2 filled any boxes, and return the count if so.
    fn check_box_painting(&mut self, t1: &TilePoint, t2: &TilePoint) -> i32 {
        let indices: Vec<usize> = self
            .boxes
            .iter()
            .enumerate()
            .filter(|(_, b)| b.matches(t1) || b.matches(t2))
            .filter(|(_, b)| b.should_update_paint(self))
            .map(|(i, _)| i)
            .collect();

        let updated = indices.len() as i32;
        for i in indices {
            self.boxes[i].painted = true;
        }

        updated
    }

    fn check_paint(&mut self, player_history: &mut VecDeque<u32>) -> ScoreUpdate {
        let mut score_change = ScoreUpdate::new();

        if let Some(end) = player_history.front() {
            if let Some(start) = player_history.iter().find(|j| *j != end) {
                // iterate from start..end and paint()

                let t1 = self.lookup_position(*start);
                let t2 = self.lookup_position(*end);
                let dx = (t2.tx - t1.tx).signum();
                let dy = (t2.ty - t1.ty).signum();
                debug_assert!(dx.abs() + dy.abs() == 1);

                let mut newly_painted = false;
                newly_painted |= self.paint(&t1);
                let mut t = t1.clone();
                while t != t2 {
                    t = t.translate(dx, dy);
                    newly_painted |= self.paint(&t);
                }

                // vertical segments give you 1, horizontal give you length
                if newly_painted {
                    if dy > 0 {
                        score_change.vertical += (t2.ty - t1.ty).abs();
                    } else {
                        score_change.horizontal += (t2.tx - t1.tx).abs();
                    }
                    score_change.num_boxes += self.check_box_painting(&t1, &t2);
                }
            }
        }

        if score_change.happened() {
            // Don't forget this location should still be in history:
            let current = player_history.front().unwrap().clone();
            player_history.clear();
            player_history.push_front(current);
        }

        score_change
    }

    pub fn paint(&mut self, tile: &TilePoint) -> bool {
        let tile = &mut self.tiles[tile.ty as usize][tile.tx as usize];
        if *tile == Tile::Painted {
            false
        } else {
            *tile = Tile::Painted;
            true
        }
    }
    pub fn make_enemy(&self, positions: Vec<u32>) -> Mob {
        let first = positions[0];
        let ai = MovementAI::EnemyLookupAI {
            next: 0,
            path: positions,
        };
        Mob::new(ai, self.lookup_position(first).to_world())
    }
    pub fn lookup_position(&self, position: u32) -> TilePoint {
        let x = position % self.width;
        let y = position / self.width;
        TilePoint::new(x as i32, y as i32)
    }
    fn get_tile(&self, tile: &TilePoint) -> Tile {
        if let Some(row) = self.tiles.get(tile.ty as usize) {
            if let Some(t) = row.get(tile.tx as usize) {
                return *t;
            }
        }
        Tile::Empty
    }
}

pub struct State {
    pub dead: bool,
    pub game_over: bool,
    pub score: i32,
    pub box_bonus: i32,
    pub player: Mob,
    pub player_start: TilePoint,
    pub enemies: Vec<Mob>,
    pub board: Board,
}

impl State {
    pub fn try_new() -> Result<State, Error> {
        let board = Board::fast_new();

        println!("Amidar Board Size: {}x{}", board.width, board.height);

        let mut enemies = Vec::new();
        for enemy_route in AMIDAR_ENEMY_POSITIONS_DATA.lines() {
            let route: Result<Vec<u32>, _> = enemy_route
                .trim()
                .split(' ')
                .map(|x| x.parse::<u32>())
                .collect();
            enemies.push(board.make_enemy(route?));
        }
        let player_start = TilePoint::new(31, 15);
        let player = Mob::new_player(player_start.to_world());

        let mut state = State {
            dead: false,
            game_over: false,
            score: 0,
            box_bonus: 50,
            player,
            player_start,
            enemies,
            board: board,
        };
        state.reset();
        Ok(state)
    }
    pub fn reset(&mut self) {
        self.player.reset(&self.player_start, &self.board);
        self.player
            .history
            .push_front(self.board.get_junction_id(&TilePoint::new(31, 18)).unwrap());
        for enemy in self.enemies.iter_mut() {
            enemy.reset(&self.player_start, &self.board);
        }
    }
    pub fn board_size(&self) -> WorldPoint {
        let th = self.board.height as i32;
        let tw = self.board.width as i32;
        TilePoint::new(tw + 1, th + 1).to_world()
    }
    pub fn update_mut(&mut self, buttons: &[Input]) {
        if let Some(score_change) = self.player.update(buttons, &mut self.board) {
            self.score += score_change.horizontal;
            // max 1 point for vertical, for some reason.
            self.score += score_change.vertical.signum();
            self.score += self.box_bonus * score_change.num_boxes;
        }

        for enemy in self.enemies.iter_mut() {
            enemy.update(&[], &mut self.board);

            if self.player.position.to_tile() == enemy.position.to_tile() {
                self.dead = true;
                break;
            }
        }

        if self.dead {
            self.reset();
            self.dead = false;
        }
    }

    pub fn draw(&self) -> Vec<Drawable> {
        let mut output = Vec::new();
        output.push(Drawable::rect(
            Color::black(),
            0,
            0,
            screen::GAME_SIZE.0,
            screen::GAME_SIZE.1,
        ));
        if self.game_over {
            return output;
        }
        let track_color = Color::RGB(148, 0, 211);
        let player_color = Color::RGB(255, 255, 153);
        let enemy_color = Color::RGB(255, 0, 153);
        let text_color = player_color;

        let (tile_w, tile_h) = screen::TILE_SIZE;
        let (offset_x, offset_y) = screen::BOARD_OFFSET;

        for (ty, row) in self.board.tiles.iter().enumerate() {
            let ty = ty as i32;
            for (tx, tile) in row.iter().enumerate() {
                let tx = tx as i32;
                let tile_color = match tile {
                    // TODO: change this color:
                    Tile::Painted => Color::white(),
                    Tile::Unpainted => track_color,
                    Tile::Empty => continue,
                };
                output.push(Drawable::rect(
                    tile_color,
                    offset_x + tx * tile_w,
                    offset_y + ty * tile_h,
                    tile_w,
                    tile_h,
                ));
            }
        }

        for inner_box in self.board.boxes.iter().filter(|b| b.painted) {
            let origin = inner_box.top_left.translate(1, 1).to_world().to_screen();
            let dest = inner_box.bottom_right.to_world().to_screen();
            let w = dest.sx - origin.sx;
            let h = dest.sy - origin.sy;

            output.push(Drawable::rect(
                Color::RGB(0xff, 0xff, 0x00),
                offset_x + origin.sx,
                offset_y + origin.sy,
                w,
                h,
            ));
        }

        let (player_x, player_y) = self.player.position.to_screen().pixels();
        let (player_w, player_h) = screen::PLAYER_SIZE;
        output.push(Drawable::rect(
            player_color,
            offset_x + player_x - 1,
            offset_y + player_y - 1,
            player_w,
            player_h,
        ));

        for enemy in self.enemies.iter() {
            let (x, y) = enemy.position.to_screen().pixels();
            let (w, h) = screen::ENEMY_SIZE;

            output.push(Drawable::rect(
                Color::RGB(0xff, 0, 0),
                offset_x + x - 1,
                offset_y + y - 1,
                w,
                h,
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_included() {
        let board_ch: Vec<Vec<char>> = AMIDAR_BOARD
            .lines()
            .map(|line| line.chars().collect::<Vec<char>>())
            .collect();
        for row in board_ch.iter() {
            assert_eq!(Some('='), row.iter().cloned().find(|c| *c == '='));
        }
    }

    #[test]
    fn board_corners() {
        let board = Board::fast_new();
        assert!(board.is_corner(0, 0));
        assert!(board.is_corner(0, 30));
        assert!(board.is_corner(31, 0));
        assert!(board.is_corner(31, 30));
    }
    #[test]
    fn player_start_position() {
        let board = Board::fast_new();
        assert_eq!(TilePoint::new(31, 15), board.lookup_position(511));
        assert!(board.get_junction_id(&TilePoint::new(31, 18)).is_some());
    }

    #[test]
    fn num_grid_boxes() {
        let board = Board::fast_new();
        let mut ordered = board.boxes.clone();
        ordered.sort_by_key(|it| it.top_left.tx + it.top_left.ty * 32);
        for gb in ordered {
            println!("Box-found: {:?}", gb.top_left);
        }
        assert_eq!(board.boxes.len(), 29);
    }
}
