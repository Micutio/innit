extern crate tcod;
extern crate rand;

use std::cmp;
use rand::Rng;
use tcod::console::*;
use tcod::colors::{self, Color};
use tcod::map::{Map as FovMap, FovAlgorithm};

// window size
const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
// target fps
const LIMIT_FPS: i32 = 20;
// constraints for field of view computing and rendering
const FOV_ALG: FovAlgorithm = FovAlgorithm::Shadow;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;
// player object
const PLAYER: usize = 0;
// object generation constraints
const MAX_ROOM_MONSTERS: i32 = 3;
// map constraints
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;
const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color { r: 130, g: 110, b: 50 };
const COLOR_DARK_GROUND: Color = Color { r: 50, g: 50, b: 150 };
const COLOR_LIGHT_GROUND: Color = Color { r: 200, g: 180, b: 50 };
// room generation constraints
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;

// objects and player

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit
}

#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    name: String,
    blocks: bool,
    alive: bool,
    chr: char,
    color: Color,
    fighter: Option<Fighter>,
    ai: Option<Ai>,
}

impl Object {

    pub fn new(x: i32, y: i32, name: &str, blocks: bool, chr: char, color: Color) -> Self {
        Object {
            x: x,
            y: y,
            name: name.into(),
            blocks: blocks,
            alive: false,
            chr: chr,
            color: color,
            fighter: None,
            ai: None,
        }
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Set the color and then draw the char that represents this object at its position.
    pub fn draw(&self, con: &mut Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.chr, BackgroundFlag::None);
    }

    /// Erase the character that represents this object
    pub fn clear(&self, con: &mut Console) {
        con.put_char(self.x, self.y, ' ', BackgroundFlag::None);
    }

    pub fn distance_to(&self, other: &Object) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
    }

    pub fn take_damage(&mut self, damage: i32) {
        // apply damage if possible
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }

        // check for death, trigger death callback function
        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self);
            }
        }
    }

    pub fn attack(&mut self, target: &mut Object) {
        // simple formula for attack damage
        let damage = self.fighter.map_or(0, |f| f.power) - target.fighter.map_or(0, |f| f.defense);
        if damage > 0 {
            // make the target take some damage
            println!("{} attacks {} for {} hit points.", self.name, target.name, damage);
            target.take_damage(damage);
        } else {
            println!("{} attacks {} but it has no effect!", self.name, target.name);
        }
    }
}

// combat related poperties and methods (monster, player, NPC)
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defense: i32,
    power: i32,
    on_death: DeathCallback,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut Object) {
        use DeathCallback::*;
        let callback: fn(&mut Object) = match self {
            Player => player_death,
            Monster => monster_death,
        };
        callback(object);
    }
}

fn player_death(player: &mut Object) {
    // the game ended!
    println!("You died!");

    // for added effect, transform the player into a corpse
    player.chr = '%';
    player.color = colors::DARK_RED;
}

fn monster_death(monster: &mut Object) {
    println!("{} is dead!", monster.name);
    monster.chr = '%';
    monster.color = colors::DARK_RED;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("remains of {}", monster.name);
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ai;

fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut[Object]) {
    // move by the given amount
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

fn player_move_or_attack(dx: i32, dy: i32, map: &Map, objects: &mut[Object]) {
    // the coordinate the player is moving to/attacking
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;
    
    // try to find an attackable object there
    let target_id = objects.iter().position(|object| {
        object.fighter.is_some() && object.pos() == (x, y)
    });

    // attack if target found, move otherwise
    match target_id {
        Some(target_id) => {
            let (player, target) = mut_two(PLAYER, target_id, objects);
            player.attack(target);
        }
        None => {
            move_by(PLAYER, dx, dy, map, objects);
        }
    }
}

fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [Object]){
    // vector from this object to the target, and distance
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;
    let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

    // normalize it to length 1 (preserving direction), then round it and
    // convert to integer so the movement is restricted to the map grid
    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, dx, dy, map, objects);
}

fn ai_take_turn(monster_id: usize, map: &Map, objects: &mut [Object], fov_map: &FovMap) {
    // A basic monster takes its turn. If you can see it, it can see you.
    let (monster_x, monster_y) = objects[monster_id].pos();
    if fov_map.is_in_fov(monster_x, monster_y) {
        if objects[monster_id].distance_to(&objects[PLAYER]) >= 2.0 {
            // move towards player if far away
            let (player_x, player_y) = objects[PLAYER].pos();
            move_towards(monster_id, player_x, player_y, map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |f| f.hp > 0) {
            // Close enough, attack! (if player is still alive)
            let (monster, player) = mut_two(monster_id, PLAYER, objects);
            monster.attack(player);
        }
    }
}

// tiles, map and rooms

#[derive(Clone, Copy, Debug)]
struct Tile {
    blocked: bool,
    block_sight: bool,
    explored: bool,
}

impl Tile {

    pub fn empty() -> Self {
        Tile { blocked: false, block_sight: false, explored: false }
    }

    pub fn wall() -> Self {
        Tile { blocked: true, block_sight: true, explored: false }
    }

}

type Map = Vec<Vec<Tile>>;

fn make_map(objects: &mut Vec<Object>) -> Map {
    // fill the map with `unblocked` tiles
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    
    // create rooms randomly
    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // random width and height
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        
        // random position without exceeding the boundaries of the map
        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        // create room and store in vector
        let new_room = Rect::new(x, y, w, h);
        let failed = rooms.iter().any(|other_room| new_room.intersects_with(other_room));

        if !failed {
            // no intersections, we have a valid room.
            create_room(new_room, &mut map);

            // add some content to the room
            place_objects(new_room, &map, objects);

            let (new_x, new_y) = new_room.center();
            if rooms.is_empty() {
                // this is the first room, save position as starting point for the player
                objects[PLAYER].set_pos(new_x, new_y);
            } else {
                // all rooms after the first:
                // connect it to the previous room with a tunnel

                // center coordinates of the previous room
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                // connect both rooms with a horizontal and a vertical tunnel - in random order
                if rand::random() {
                    // move horizontally, then vertically
                    create_h_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    // move vertically, then horizontally
                    create_v_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, new_x, new_y, &mut map);
                }
            }
            // finally, append new room to list
            rooms.push(new_room);
        }
    }
    
    map
}

// data structures for room generation
#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect { x1: x, y1: y, x2: x + w, y2: y + h}
    }

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        (center_x, center_y)
    }

    /// Return true if this rect intersects with another one.
    pub fn intersects_with(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2) && ( self.x2 >= other.x1) &&
            (self.y1 <= other.y2) && (self.y2 >= other.y1)
    }
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    // choose random number of monsters
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);
    for _ in 0..num_monsters {
        // choose random spot for this monster
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);
        
        if !is_blocked(x, y, map, objects) {
            let mut monster = if rand::random::<f32>() < 0.8 {
                let mut orc = Object::new(x, y, "orc", true ,'o', colors::DESATURATED_GREEN);
                orc.fighter = Some(Fighter{max_hp: 10, hp: 10, defense: 0, power: 3, on_death: DeathCallback::Monster});
                orc.ai = Some(Ai);
                orc
            } else {
                let mut troll = Object::new(x, y, "troll", true, 'T', colors::DARKER_GREEN);
                troll.fighter = Some(Fighter{max_hp: 16, hp: 16, defense: 1, power: 4, on_death: DeathCallback::Monster});
                troll.ai = Some(Ai);
                troll
            };

            monster.alive = true;
            objects.push(monster);
        }
    }
}

/// Mutably borrow two *separate* elements from the given slice.
/// Panics when the indexes are equal or out of bounds.
fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first_index != second_index);
    let split_at_index = cmp::max(first_index, second_index);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    // first test the map tile
    if map[x as usize][y as usize].blocked {
        return true;
    }
    // now check for any blocking objects
    objects.iter().any(|object| {
        object.blocks && object.pos() == (x, y)
    })
}

/// Render all objects and tiles.
fn render_all(root: &mut Root, con: &mut Offscreen, objects: &[Object], map: &mut Map, fov_map: &mut FovMap, fov_recompute: bool) {
    
    if fov_recompute {
        // recompute fov if needed (the player moved or something)
        let player = &objects[PLAYER];
        fov_map.compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALG);
    }

    // go through all tiles and set their background color
    for y in 0..MAP_HEIGHT {
        for x in  0..MAP_WIDTH {
            let visible = fov_map.is_in_fov(x, y);
            let wall = map[x as usize][y as usize].block_sight;
            let tile_color = match (visible, wall) {
                // outside field of view:
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                // inside fov:
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };

            let explored = &mut map[x as usize][y as usize].explored;
            if visible {
                *explored = true;
            }
            if *explored {
                // show explored tiles only (any visible tile is explored already)
                con.set_char_background(x, y, tile_color, BackgroundFlag::Set);
            }
        }
    }
    
    let mut to_draw: Vec<_> = objects.iter().filter(|o| fov_map.is_in_fov(o.x, o.y)).collect();
    // sort, so that non-blocking objects com first
    to_draw.sort_by(|o1, o2| {o1.blocks.cmp(&o2.blocks) });
    // draw the objects in the list
    for object in &to_draw {
        if fov_map.is_in_fov(object.x, object.y) {
            object.draw(con);
        }
    }
    
    // show the player's status
    root.set_default_foreground(colors::WHITE);
    if let Some(fighter) = objects[PLAYER].fighter {
        root.print_ex(1, SCREEN_HEIGHT - 2, BackgroundFlag::None,
                      TextAlignment::Left, format!("HP: {}/{}", fighter.hp, fighter.max_hp));
    }

    // blit contents of offscreen console to root console and present it
    blit(con, (0, 0), (MAP_WIDTH, MAP_HEIGHT), root, (0, 0), 1.0, 1.0);
}


/// Handle user input
fn handle_keys(root: &mut Root, map: &Map, objects: &mut [Object]) -> PlayerAction {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;
    use PlayerAction::*;
    
    let key = root.wait_for_keypress(true);
    let player_alive = objects[PLAYER].alive;
    match (key, player_alive) {
        // toggle fullscreen
        (Key { code: Enter, alt: true, .. }, _) => {
            let fullscreen = root.is_fullscreen();
            root.set_fullscreen(!fullscreen);
            DidntTakeTurn
        }
 
        // exit game
        (Key { code: Escape, .. }, _) => return Exit,

        // handle movement
        (Key { code: Up, .. }, true) => {
            player_move_or_attack(0, -1, map, objects);
            TookTurn
        }
        (Key { code: Down, .. }, true) => {
            player_move_or_attack(0, 1, map, objects);
            TookTurn
        }
        (Key { code: Left, .. }, true) => {
            player_move_or_attack(-1, 0, map, objects);
            TookTurn
        }
        (Key { code: Right, ..}, true) => {
            player_move_or_attack(1, 0, map, objects);
            TookTurn
        }
        
        _ => DidntTakeTurn
    }
}

fn main() {
    let mut root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("roguelike")
        .init();

    let mut con = Offscreen::new(SCREEN_WIDTH, SCREEN_HEIGHT);

    tcod::system::set_fps(LIMIT_FPS);

    // create object representing the player
    let mut player = Object::new(0, 0, "player", true, '@', colors::WHITE);
    player.alive = true;
    player.fighter = Some(Fighter{max_hp: 30, hp: 30, defense: 2, power: 5, on_death: DeathCallback::Player});

    // create array holding all objects
    let mut objects = vec![player];

    // create map and player starting position
    let mut map = make_map(&mut objects);

    // init fov map
    let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            fov_map.set(x, y,
                        !map[x as usize][y as usize].block_sight,
                        !map[x as usize][y as usize].blocked);
        }
    }

    let mut previous_player_position = (-1, -1);

    while !root.window_closed() {
        // render objects and map
        let fov_recompute = previous_player_position != (objects[PLAYER].x, objects[PLAYER].y);
        render_all(&mut root, &mut con, &objects, &mut map, &mut fov_map, fov_recompute);

        root.flush(); // draw everything on the window at once
        
        // erase all objects from their old locations before they move
        for object in &objects {
            object.clear(&mut con);
        }

        // handle keys and exit game if needed
        previous_player_position = objects[PLAYER].pos();
        let player_action = handle_keys(&mut root, &map, &mut objects);        
        if player_action == PlayerAction::Exit {
            break
        }
        
        if objects[PLAYER].alive && player_action != PlayerAction::DidntTakeTurn {
            for id in 0..objects.len() {
                if objects[id].ai.is_some() {
                    ai_take_turn(id, &map, &mut objects, &fov_map);
                }
            }
        }

    }
}
