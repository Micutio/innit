use tcod::chars;
use tcod::colors::{self, Color};
use tcod::console::*;
use tcod::map::FovAlgorithm;

use crate::core::game_env::GameEnv;
use crate::core::game_objects::GameObjects;
use crate::core::game_state::{GameState, MessageLog, MsgClass, ObjectProcResult};
use crate::core::position::Position;
use crate::core::world::world_gen::is_explored;
use crate::entity::genetics::{Dna, TraitFamily};
use crate::entity::object::Object;
use crate::game::{game_loop, load_game, new_game, save_game};
use crate::game::{WORLD_HEIGHT, WORLD_WIDTH};
use crate::ui::color_palette::*;
use crate::ui::dialog::*;
use crate::ui::game_input::{GameInput, UiAction};

// game window properties
pub const SCREEN_WIDTH: i32 = 81;
pub const SCREEN_HEIGHT: i32 = 50;
const LIMIT_FPS: i32 = 60; // target fps

// field of view algorithm parameters
const FOV_ALG: FovAlgorithm = FovAlgorithm::Shadow;
const FOV_LIGHT_WALLS: bool = true;

// ui and menu constraints
pub const BAR_WIDTH: i32 = 20;
pub const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;

use crate::entity::action::{Action, Target, TargetCategory};
use crate::util::modulus;
/// Field of view mapping.
pub use tcod::map::Map as FovMap;

/// GameFrontend holds the core components for game's input and output processing.
pub struct GameFrontend {
    pub root: Root,
    pub con: Offscreen,
    pub btm_panel: Offscreen,
    pub dna_panel: Offscreen,
    pub fov: FovMap,
    pub input: Option<GameInput>,
    pub coloring: ColorPalette,
    is_light_mode: bool,
}

impl GameFrontend {
    /// Initialize the game frontend:
    ///     - load assets, like fonts etc.
    ///     - set ui window size
    ///     - set ui window title
    ///     - set fps
    ///     - init permanent ui components
    pub fn new() -> Self {
        let root = Root::initializer()
            // .font("assets/tilesets/terminal16x16_gs_ro.png", FontLayout::AsciiInRow)
            .font(
                // "assets/tilesets/yayo_12x12.png",
                "assets/tilesets/rex_paint_14x14.png",
                FontLayout::AsciiInRow,
            )
            // .font("assets/tilesets/zilk_16x16.png", FontLayout::AsciiInRow)
            .font_type(FontType::Greyscale)
            .size(SCREEN_WIDTH, SCREEN_HEIGHT)
            .title("Innit alpha v0.0.2")
            .init();

        tcod::system::set_fps(LIMIT_FPS);

        GameFrontend {
            root,
            con: Offscreen::new(SCREEN_WIDTH, SCREEN_HEIGHT),
            btm_panel: Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT),
            dna_panel: Offscreen::new(1, SCREEN_HEIGHT - PANEL_HEIGHT),
            fov: FovMap::new(WORLD_WIDTH, WORLD_HEIGHT),
            input: None,
            // TODO: Save light and dark setting to config
            coloring: ColorPalette::new_dark(),
            is_light_mode: true,
        }
    }

    pub fn toggle_dark_light_mode(&mut self) {
        if self.is_light_mode {
            self.is_light_mode = false;
            self.coloring = ColorPalette::new_dark();
        } else {
            self.is_light_mode = true;
            self.coloring = ColorPalette::new_light();
        }
    }
}

/// Specification of animations and their parameters.
// TODO: Move this to its own module.
#[derive(PartialEq, Debug)]
pub enum AnimationType {
    /// Gradual transition of the world hue and or brightness
    ColorTransition,
    /// A cell flashes with a specific character.
    /// Example: flash a red 'x' over an object to indicate a hit.
    FlashEffect,
}

/// Main menu of the game.
/// Display a background image and three options for the player to choose
///     - starting a new game
///     - loading an existing game
///     - quitting the game
pub fn main_menu(env: GameEnv, frontend: &mut GameFrontend) {
    let img = tcod::image::Image::from_file("assets/menu_background_pixelized_title.png")
        .expect("Background image not found");

    while !frontend.root.window_closed() {
        // show the background image, at twice the regular console resolution
        tcod::image::blit_2x(&img, (0, 0), (-1, -1), &mut frontend.root, (0, 0));
        // let mut x: u8 = 0;
        // for i in 0..16 {
        //     for j in 0..16 {
        //         game_frontend
        //             .root
        //             .put_char_ex(i, j, x as u8 as char, colors::WHITE, colors::BLUE);
        //         if x < 255 {
        //             x += 1;
        //         }
        //     }
        // }

        frontend
            .root
            .set_default_foreground(frontend.coloring.bg_dialog);
        frontend
            .root
            .set_default_background(frontend.coloring.bg_dialog);
        frontend.root.print_ex(
            SCREEN_WIDTH / 2,
            SCREEN_HEIGHT - 2,
            BackgroundFlag::None,
            TextAlignment::Center,
            "By Michael Wagner",
        );

        // show options and wait for the player's choice
        let choices = &["Play a new game", "Continue last game", "Quit"];
        let choice = menu(frontend, &mut None, "main menu", choices, 24);

        match choice {
            Some(0) => {
                // start new game
                let (mut state, mut objects) = new_game(env, frontend);
                // initialize_fov(game_frontend, &mut objects);
                let mut input = GameInput::new();
                init_object_visuals(&mut state, frontend, &input, &mut objects);
                input.start_concurrent_input();
                game_loop(&mut state, frontend, &mut input, &mut objects);
            }
            Some(1) => {
                // load game from file
                match load_game() {
                    Ok((mut state, mut objects)) => {
                        // initialize_fov(game_frontend, &mut objects);
                        let mut input = GameInput::new();
                        init_object_visuals(&mut state, frontend, &input, &mut objects);
                        input.start_concurrent_input();
                        game_loop(&mut state, frontend, &mut input, &mut objects);
                    }
                    Err(_e) => {
                        msg_box(frontend, &mut None, "", "\nNo saved game to load\n", 24);
                        continue;
                    }
                }
            }
            Some(2) => {
                // quit
                break;
            }
            _ => {}
        }
    }
}

/// Initialize the field of view map with the world tiles contained in game_objects.
fn initialize_fov(frontend: &mut GameFrontend, objects: &mut GameObjects) {
    // init fov map
    for y in 0..WORLD_HEIGHT {
        for x in 0..WORLD_WIDTH {
            match objects.get_tile_at(x as usize, y as usize) {
                Some(object) => {
                    frontend.fov.set(
                        x as i32,
                        y as i32,
                        !object.physics.is_blocking_sight,
                        !object.physics.is_blocking,
                    );
                }
                None => {
                    panic!("[game_frontend] Error initializing fov");
                }
            }
        }
    }
    // unexplored areas start black (which is the default background color)
    frontend.con.clear();
    frontend
        .con
        .set_default_background(frontend.coloring.bg_world);
}

fn recompute_fov(state: &GameState, frontend: &mut GameFrontend, objects: &GameObjects) {
    if let Some(ref player) = objects[state.current_player_index] {
        // println!("recomputing FOV: {}", player.sensors.sensing_range);
        frontend.fov.compute_fov(
            player.pos.x,
            player.pos.y,
            player.sensors.sensing_range,
            FOV_LIGHT_WALLS,
            FOV_ALG,
        );
    }
}

/// Initialize the player's field of view and render objects + ui for the start of the game.
fn init_object_visuals(
    state: &mut GameState,
    frontend: &mut GameFrontend,
    input: &GameInput,
    objects: &mut GameObjects,
) {
    initialize_fov(frontend, objects);
    recompute_fov(state, frontend, objects);
    re_render(state, frontend, objects, &input.names_under_mouse);
}

/// Update the player's field of view and updated which tiles are visible/explored.
fn update_visibility(state: &GameState, frontend: &mut GameFrontend, objects: &mut GameObjects) {
    // go through all tiles and set their background color
    let mut player_pos: Position = Position::new(0, 0);
    let mut player_sensing_range: f32 = 0.0;
    if let Some(ref mut player) = objects[state.current_player_index] {
        player_pos.set(player.pos.x, player.pos.y);
        player_sensing_range = player.sensors.sensing_range as f32;
        player.visual.color = frontend.coloring.player;
    }

    // let col_wall_out_fov = game_frontend.coloring.get_col_wall_out_fov();
    // let col_wall_in_fov = game_frontend.coloring.get_col_wall_in_fov();
    // let col_ground_out_fov = game_frontend.coloring.get_col_ground_out_fov();
    // let col_ground_in_fov = game_frontend.coloring.get_col_ground_in_fov();
    let bwft = frontend.coloring.bg_wall_fov_true;
    let bwff = frontend.coloring.bg_wall_fov_false;
    let bgft = frontend.coloring.bg_ground_fov_true;
    let bgff = frontend.coloring.bg_ground_fov_false;
    let fwft = frontend.coloring.fg_wall_fov_true;
    let fwff = frontend.coloring.fg_wall_fov_false;
    let fgft = frontend.coloring.fg_ground_fov_true;
    let fgff = frontend.coloring.fg_ground_fov_false;

    for y in 0..WORLD_HEIGHT {
        for x in 0..WORLD_WIDTH {
            let visible = frontend.fov.is_in_fov(x, y);
            if let Some(ref mut tile_object) = objects.get_tile_at(x as usize, y as usize) {
                let wall = tile_object.physics.is_blocking_sight;

                // set tile foreground and background colors
                let (tile_color_fg, tile_color_bg) = match (visible, wall) {
                    // outside field of view:
                    (false, true) => (fwff, bwff),
                    (false, false) => (fgff, bgff),
                    // inside fov:
                    // (true, true) => COLOR_LIGHT_WALL,
                    (true, true) => (
                        colors::lerp(
                            fwft,
                            fwff,
                            tile_object.pos.distance(&player_pos) / player_sensing_range,
                        ),
                        colors::lerp(
                            bwft,
                            bwff,
                            tile_object.pos.distance(&player_pos) / player_sensing_range,
                        ),
                    ),
                    // (true, false) => COLOR_ground_in_fov,
                    (true, false) => (
                        colors::lerp(
                            fgft,
                            fgff,
                            tile_object.pos.distance(&player_pos) / player_sensing_range,
                        ),
                        colors::lerp(
                            bgft,
                            bgff,
                            tile_object.pos.distance(&player_pos) / player_sensing_range,
                        ),
                    ),
                };

                if let Some(tile) = &mut tile_object.tile {
                    if visible {
                        tile.is_explored = true;
                    }
                    if tile.is_explored {
                        // show explored tiles only (any visible tile is explored already)
                        tile_object.visual.color = tile_color_fg;
                        frontend
                            .con
                            .set_char_background(x, y, tile_color_bg, BackgroundFlag::Set);
                    }
                }
            }
        }
    }
}

pub fn process_visual_feedback(
    state: &mut GameState,
    frontend: &mut GameFrontend,
    input: &GameInput,
    objects: &mut GameObjects,
    proc_result: ObjectProcResult,
) {
    match proc_result {
        // no action has been performed, repeat the turn and try again
        ObjectProcResult::NoAction => {}

        // action has been completed, but nothing needs to be done about it
        ObjectProcResult::NoFeedback => {}

        // the player's FOV has been updated, thus we also need to re-render
        ObjectProcResult::UpdateFOV => {
            recompute_fov(state, frontend, objects);
            re_render(state, frontend, objects, &input.names_under_mouse);
        }

        // the player hasn't moved but something happened within fov
        ObjectProcResult::ReRender => {
            re_render(state, frontend, objects, &input.names_under_mouse);
        }

        ObjectProcResult::Animate { anim_type: _ } => {
            // TODO: Play animation.
            info!("animation");
        }

        ObjectProcResult::Message { msg, class, origin } => {
            if frontend.fov.is_in_fov(origin.x, origin.y) {
                state.log.add(msg, class);
            }
        }
        ObjectProcResult::CheckEnterFOV => {}
    }
}

/// Render all objects, the menu
pub fn re_render(
    state: &mut GameState,
    frontend: &mut GameFrontend,
    objects: &mut GameObjects,
    names_under_mouse: &str,
) {
    // clear the screen of the previous frame
    frontend.con.clear();
    // render objects and map
    // step 1/2: update visibility of objects and world tiles
    update_visibility(state, frontend, objects);
    // step 2/2: render everything
    render_all(frontend, state, objects, names_under_mouse);

    // draw everything on the window at once
    frontend.root.flush();
}

/// Render all objects.
/// Right now this happens because we are updating explored tiles here too.
/// Is there a way to auto-update explored and visible tiles/objects when the player moves?
/// But visibility can also be influenced by other things.
fn render_all(
    frontend: &mut GameFrontend,
    state: &mut GameState,
    objects: &GameObjects,
    names_under_mouse: &str,
) {
    render_objects(&state.env, frontend, objects);
    render_ui(state, frontend, objects, names_under_mouse);
    blit_consoles(frontend);
}

pub fn render_objects(env: &GameEnv, frontend: &mut GameFrontend, objects: &GameObjects) {
    let mut to_draw: Vec<&Object> = objects
        .get_vector()
        .iter()
        .flatten()
        .filter(|o| {
            // FIXME: there must be a better way than using `and_then`.
            frontend.fov.is_in_fov(o.pos.x, o.pos.y)
                || o.physics.is_always_visible
                || (o.tile.is_some() && *o.tile.as_ref().and_then(is_explored).unwrap())
                || (o.tile.is_some() && env.debug_mode)
        })
        .collect();

    // sort, so that non-blocking objects come first
    to_draw.sort_by(|o1, o2| o1.physics.is_blocking.cmp(&o2.physics.is_blocking));
    // draw the objects in the list
    for object in &to_draw {
        draw_object(object, &mut frontend.con);
    }
}

/// Set the color and then draw the char that represents this object at its position.
fn draw_object(object: &Object, con: &mut dyn Console) {
    con.set_default_foreground(object.visual.color);
    con.put_char(
        object.pos.x,
        object.pos.y,
        object.visual.character,
        BackgroundFlag::None,
    );
}

pub fn blit_consoles(frontend: &mut GameFrontend) {
    // blit contents of offscreen console to root console and present it
    blit(
        &frontend.con,
        (0, 0),
        (WORLD_WIDTH, WORLD_HEIGHT),
        &mut frontend.root,
        (0, 0),
        1.0,
        1.0,
    );

    // blit contents of `game_frontend.btm_panel` to the root console
    blit(
        &frontend.btm_panel,
        (0, 0),
        (SCREEN_WIDTH, SCREEN_HEIGHT),
        &mut frontend.root,
        (0, PANEL_Y),
        1.0,
        1.0,
    );

    // blit contents of `game_frontend.btm_panel` to the root console
    blit(
        &frontend.dna_panel,
        (0, 0),
        (SCREEN_WIDTH, SCREEN_HEIGHT - PANEL_HEIGHT),
        &mut frontend.root,
        (SCREEN_WIDTH - 1, 0),
        1.0,
        1.0,
    );
}

/// Render the user interface, consisting of:
///     - health bar
///     - player stats
///     - message log
///     - objects names under mouse cursor
/// Add all ui elements to the panel component of the frontend.
fn render_ui(
    state: &mut GameState,
    frontend: &mut GameFrontend,
    objects: &GameObjects,
    names_under_mouse: &str,
) {
    render_btm_panel(&frontend.coloring, &mut frontend.btm_panel);

    // show player's stats
    if let Some(ref player) = objects[state.current_player_index] {
        render_bar(
            &mut frontend.btm_panel,
            1,
            1,
            BAR_WIDTH,
            "HP",
            player.actuators.hp,
            player.actuators.max_hp,
            frontend.coloring.fg_dialog,
            colors::DARKER_RED,
            colors::DARKEST_RED,
        );
        render_bar(
            &mut frontend.btm_panel,
            1,
            2,
            BAR_WIDTH,
            "ENERGY",
            player.processors.energy,
            player.processors.energy_storage,
            frontend.coloring.fg_dialog,
            frontend.coloring.yellow,
            colors::DARKER_YELLOW,
        );
        render_textfield(
            &mut frontend.btm_panel,
            &frontend.coloring,
            colors::DARK_GREY,
            1,
            3,
            BAR_WIDTH,
            'P',
            &player.get_primary_action(Target::Center).get_identifier(),
        );
        render_textfield(
            &mut frontend.btm_panel,
            &frontend.coloring,
            colors::DARK_GREY,
            1,
            4,
            BAR_WIDTH,
            'S',
            &player.get_secondary_action(Target::Center).get_identifier(),
        );
        render_textfield(
            &mut frontend.btm_panel,
            &frontend.coloring,
            colors::DARK_GREY,
            1,
            5,
            BAR_WIDTH,
            '1',
            &player.get_quick1_action().get_identifier(),
        );

        render_dna_panel(&mut frontend.dna_panel, &frontend.coloring, &player.dna);

        // show names of objects under the mouse
        if !names_under_mouse.is_empty() {
            frontend
                .btm_panel
                .set_default_foreground(colors::LIGHT_GREY);
            frontend.btm_panel.print_ex(
                2,
                0,
                BackgroundFlag::None,
                TextAlignment::Left,
                names_under_mouse,
            );
            frontend
                .btm_panel
                .set_default_foreground(frontend.coloring.fg_dialog_border);
            frontend
                .btm_panel
                .put_char(1, 0, '\u{b9}', BackgroundFlag::Set);
            frontend.btm_panel.put_char(
                (names_under_mouse.len() + 2) as i32,
                0,
                '\u{cc}',
                BackgroundFlag::Set,
            );
        }

        // print game messages, one line at a time
        let mut y = MSG_HEIGHT as i32;
        for (ref msg, class) in &mut state.log.iter().rev() {
            let msg_height = frontend
                .btm_panel
                .get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
            y -= msg_height;
            if y < 1 {
                break;
            }

            // TODO: Use custom color scheme instead.
            let color = match class {
                MsgClass::Alert => colors::DARK_RED,
                MsgClass::Info => colors::WHITE,
                MsgClass::Action => colors::AZURE,
                MsgClass::Story => colors::DESATURATED_CYAN,
            };

            frontend.btm_panel.set_default_foreground(color);
            frontend.btm_panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);
        }
    }
}

fn render_btm_panel(coloring: &ColorPalette, panel: &mut Offscreen) {
    // prepare to render the GUI panel
    panel.set_default_background(coloring.bg_dialog);
    panel.clear();

    // set panel borders
    // set background and foreground colors
    for x in 0..SCREEN_WIDTH {
        for y in 0..PANEL_HEIGHT {
            panel.set_char_background(x, y, coloring.bg_dialog, BackgroundFlag::Set);
            panel.set_char_foreground(x, y, coloring.fg_dialog_border);
            panel.set_char(x, y, ' ');
        }
    }

    // render horizontal borders
    for x in 0..SCREEN_WIDTH - 1 {
        panel.set_char(x, 0, chars::DHLINE);
        panel.set_char(x, PANEL_HEIGHT - 1, chars::HLINE);
    }
    // render vertical borders
    for y in 0..PANEL_HEIGHT - 1 {
        panel.set_char(0, y, chars::VLINE);
        panel.set_char(SCREEN_WIDTH - 1, y, chars::VLINE);
    }

    // render corners
    panel.set_char(0, 0, '\u{d5}');
    // panel.set_char(SCREEN_WIDTH - 1, 0, '\u{b8}');
    panel.set_char(SCREEN_WIDTH - 1, 0, '\u{b5}');
    panel.set_char(0, PANEL_HEIGHT - 1, chars::SW);
    panel.set_char(SCREEN_WIDTH - 1, PANEL_HEIGHT - 1, chars::SE);

    panel.set_default_foreground(coloring.fg_dialog);
}

pub fn handle_meta_actions(
    state: &mut GameState,
    frontend: &mut GameFrontend,
    objects: &mut GameObjects,
    input: &mut Option<&mut GameInput>,
    action: UiAction,
) -> bool {
    // TODO: Screens for key mapping, primary and secondary action selection, dna operations.
    debug!("received action {:?}", action);
    match action {
        UiAction::ExitGameLoop => {
            let result = save_game(state, objects);
            result.unwrap();
            return true;
        }
        UiAction::ToggleDarkLightMode => {
            frontend.toggle_dark_light_mode();
            recompute_fov(state, frontend, objects);
            initialize_fov(frontend, objects);
            re_render(state, frontend, objects, "");
        }
        UiAction::CharacterScreen => {
            show_character_screen(state, frontend, input, objects);
        }

        UiAction::Fullscreen => {
            let fullscreen = frontend.root.is_fullscreen();
            frontend.root.set_fullscreen(!fullscreen);
            initialize_fov(frontend, objects);
        }
        UiAction::ChoosePrimaryAction => {
            if let Some(ref mut player) = objects[state.current_player_index] {
                if let Some(a) = get_available_action(
                    frontend,
                    player,
                    "primary",
                    &[
                        TargetCategory::Any,
                        TargetCategory::EmptyObject,
                        TargetCategory::BlockingObject,
                    ],
                ) {
                    player.set_primary_action(a);
                }
            }
        }
        UiAction::ChooseSecondaryAction => {
            if let Some(ref mut player) = objects[state.current_player_index] {
                if let Some(a) = get_available_action(
                    frontend,
                    player,
                    "secondary",
                    &[
                        TargetCategory::Any,
                        TargetCategory::EmptyObject,
                        TargetCategory::BlockingObject,
                    ],
                ) {
                    player.set_secondary_action(a);
                }
            }
        }
        UiAction::ChooseQuick1Action => {
            if let Some(ref mut player) = objects[state.current_player_index] {
                if let Some(a) =
                    get_available_action(frontend, player, "secondary", &[TargetCategory::None])
                {
                    player.set_quick1_action(a);
                }
            }
        }
        UiAction::ChooseQuick2Action => {
            if let Some(ref mut player) = objects[state.current_player_index] {
                if let Some(a) =
                    get_available_action(frontend, player, "secondary", &[TargetCategory::None])
                {
                    player.set_quick1_action(a);
                }
            }
        }
    }
    re_render(state, frontend, objects, "");
    false
}

fn get_available_action(
    frontend: &mut GameFrontend,
    obj: &mut Object,
    action_id: &str,
    targets: &[TargetCategory],
) -> Option<Box<dyn Action>> {
    let choices: Vec<String> = obj
        .actuators
        .actions
        .iter()
        .chain(obj.processors.actions.iter())
        .chain(obj.sensors.actions.iter())
        .filter(|a| targets.contains(&a.get_target_category()))
        .map(|a| a.get_identifier())
        .collect();

    if choices.is_empty() {
        debug!("No choices available!");
        return None;
    }
    // show options and wait for the obj's choice
    let choice = menu(
        frontend,
        &mut None,
        format!("choose {}", action_id).as_str(),
        choices.as_slice(),
        24,
    );

    if let Some(c) = choice {
        obj.actuators
            .actions
            .iter()
            .chain(obj.processors.actions.iter())
            .chain(obj.sensors.actions.iter())
            .find(|a| a.get_identifier().eq(&choices[c]))
            .cloned()
    } else {
        None
    }
}

/// Render a generic progress or status bar in the UI.
fn render_dna_short(
    panel: &mut Offscreen,
    coloring: &ColorPalette,
    x: i32,
    y: i32,
    total_width: i32,
    dna: &Dna,
) {
    // get sensor/processor/actuator counts
    let sensor_count = dna
        .simplified
        .iter()
        .filter(|x| x.trait_family == TraitFamily::Sensing)
        .count();
    let processor_count = dna
        .simplified
        .iter()
        .filter(|x| x.trait_family == TraitFamily::Processing)
        .count();
    let actuator_count = dna
        .simplified
        .iter()
        .filter(|x| x.trait_family == TraitFamily::Actuating)
        .count();
    let maximum = dna.simplified.len();
    // render a bar (HP, EXP, etc)
    let sensor_width = (sensor_count as f32 / maximum as f32 * total_width as f32) as i32;
    let processor_width = (processor_count as f32 / maximum as f32 * total_width as f32) as i32;
    let actuator_width = (actuator_count as f32 / maximum as f32 * total_width as f32) as i32;

    // render each super trait count
    panel.set_default_background(coloring.cyan);
    if sensor_width > 0 {
        panel.rect(x, y, sensor_width, 1, false, BackgroundFlag::Screen);
    }
    // render each super trait count
    panel.set_default_background(coloring.magenta);
    if processor_width > 0 {
        panel.rect(
            x + sensor_width,
            y,
            processor_width,
            1,
            false,
            BackgroundFlag::Screen,
        );
    }
    // render each super trait count
    panel.set_default_background(coloring.yellow);
    if actuator_width > 0 {
        panel.rect(
            x + sensor_width + processor_width,
            y,
            actuator_width,
            1,
            false,
            BackgroundFlag::Screen,
        );
    }

    // put some text in the center
    panel.set_default_foreground(coloring.fg_dialog);
    panel.print_ex(
        x + total_width / 2,
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        "Traits".to_string(),
    );
}

/// Render a generic progress or status bar in the UI.
fn render_dna_long(
    panel: &mut Offscreen,
    coloring: &ColorPalette,
    x: i32,
    y: i32,
    total_width: i32,
    dna: &Dna,
) {
    let traits_len = dna.simplified.len();
    let bar_width = (total_width as f32 / traits_len as f32) as i32;
    let mut offset = 0;
    // println!("number of traits {}, bar width {}", traits_len, bar_width);
    for g_trait in dna.simplified.iter() {
        match g_trait.trait_family {
            TraitFamily::Sensing => panel.set_default_background(coloring.cyan),
            TraitFamily::Processing => panel.set_default_background(coloring.magenta),
            TraitFamily::Actuating => panel.set_default_background(coloring.yellow),
            TraitFamily::Junk => panel.set_default_background(colors::GREY),
        }
        panel.rect(x + offset, y, bar_width, 1, false, BackgroundFlag::Screen);
        offset += bar_width;
    }

    // put some text in the center
    panel.set_default_foreground(coloring.fg_dialog);
    panel.print_ex(
        x + total_width / 2,
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        "DNA".to_string(),
    );
}

fn render_dna_panel(panel: &mut Offscreen, coloring: &ColorPalette, dna: &Dna) {
    panel.set_default_background(coloring.bg_dialog);
    panel.set_default_foreground(coloring.fg_dialog_border);
    let top_offset = 4;

    for y in 0..SCREEN_HEIGHT - PANEL_HEIGHT {
        panel.put_char(0, y, ' ', BackgroundFlag::Set);
    }

    panel.put_char(0, 0, 'D', BackgroundFlag::Set);
    panel.put_char(0, 1, 'N', BackgroundFlag::Set);
    panel.put_char(0, 2, 'A', BackgroundFlag::Set);
    panel.put_char(0, 3, '\u{c1}', BackgroundFlag::Set);

    panel.put_char(
        0,
        SCREEN_HEIGHT - PANEL_HEIGHT - 1,
        '\u{c2}',
        BackgroundFlag::Set,
    );

    for (vert_offset, g_trait) in dna.simplified.iter().enumerate() {
        let col: Color = match g_trait.trait_family {
            TraitFamily::Sensing => coloring.cyan,
            TraitFamily::Processing => coloring.magenta,
            TraitFamily::Actuating => coloring.yellow,
            TraitFamily::Junk => colors::GREY,
        };
        panel.set_char_foreground(0, (vert_offset as i32) + top_offset, col);
        // panel.set_char(0, (vert_offset as i32) + top_offset, '\u{ba}');
        let c: char = if modulus(vert_offset, 2) == 0 {
            '\u{1f}'
        } else {
            '\u{1e}'
        };
        panel.set_char(0, (vert_offset as i32) + top_offset, c);
    }
}

/// Render a generic progress or status bar in the UI.
#[allow(clippy::too_many_arguments)]
fn render_bar(
    panel: &mut Offscreen,
    x: i32,
    y: i32,
    total_width: i32,
    name: &str,
    value: i32,
    maximum: i32,
    text_color: Color,
    bar_color: Color,
    back_color: Color,
) {
    // render a bar (HP, EXP, etc)
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // render background first
    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Set);

    // now render bar on top
    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Set);
    }

    // finally some centered text with the values
    panel.set_default_foreground(text_color);
    panel.print_ex(
        x + total_width / 2,
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        &format!("{}: {}/{}", name, value, maximum),
    );
}

#[allow(clippy::too_many_arguments)]
fn render_textfield(
    panel: &mut Offscreen,
    coloring: &ColorPalette,
    back_color: Color,
    x: i32,
    y: i32,
    width: i32,
    id: char,
    text: &str,
) {
    panel.set_default_background(coloring.bg_dialog);
    panel.put_char(x, y, id, BackgroundFlag::Set);
    panel.set_default_background(back_color);
    panel.rect(x + 2, y, width - 2, 1, false, BackgroundFlag::Set);
    panel.print_ex(
        x + 2 + ((width - 2) / 2),
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        text,
    );
}
