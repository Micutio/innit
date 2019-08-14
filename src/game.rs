//! The top level representation of the game. Here the major game components are constructed and
//! the game loop is executed.

use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};

use tcod::colors;

use crate::core::game_objects::GameObjects;
use crate::core::game_state::{GameState, MessageLog};
use crate::entity::dna::get_player_action;
use crate::entity::fighter::{DeathCallback, Fighter};
use crate::entity::object::Object;
use crate::ui::game_frontend::{handle_meta_actions, process_visual_feedback, GameFrontend};
use crate::ui::game_input::{GameInput, PlayerInput};
use crate::ui::player::PLAYER;

const SAVEGAME: &str = "data/savegame";

// world constraints
pub const WORLD_WIDTH: i32 = 80;
pub const WORLD_HEIGHT: i32 = 43;

/// Create a new game by instantiating the game engine, game state and object vector.
pub fn new_game() -> (GameState, GameObjects) {
    // create object representing the player
    let mut player = Object::new(0, 0, "player", '@', colors::WHITE, true, false, false);
    player.alive = true;
    player.fighter = Some(Fighter {
        base_max_hp:  100,
        hp:           100,
        base_defense: 1,
        base_power:   2,
        on_death:     DeathCallback::Player,
        xp:           0,
    });

    // create array holding all GameObjects
    let mut objects = GameObjects::new();
    objects.set_player(player);
    let level = 1;

    // create game state holding most game-relevant information
    //  - also creates map and player starting position
    let mut game_state = GameState::new(&mut objects, level);

    // a warm welcoming message
    game_state.log.add(
        "Welcome microbe! You're innit now. Beware of bacteria and viruses",
        colors::RED,
    );

    (game_state, objects)
}

/// Central function of the game.
/// - process player input
/// - render game world
/// - let NPCs take their turn
pub fn game_loop(
    game_state: &mut GameState,
    game_frontend: &mut GameFrontend,
    game_input: &mut GameInput,
    game_objects: &mut GameObjects,
) {
    while !game_frontend.root.window_closed() {
        // ensure that the player action from previous turns is consumed
        assert!(game_input.is_action_consumed());

        // let the game engine process an object
        let process_result = game_state.process_object(game_objects, &game_frontend.fov);
        process_visual_feedback(
            game_state,
            game_frontend,
            game_input,
            game_objects,
            process_result,
        );

        // once processing is done, check whether we have a new user input
        game_input.check_for_player_actions(game_state, game_frontend, game_objects);

        // distinguish between in-game action and ui (=meta) actions
        // TODO: Enable multi-key/mouse actions e.g., select target & attack.
        match game_input.get_next_action() {
            Some(PlayerInput::MetaInput(meta_action)) => {
                debug!("process meta action: {:#?}", meta_action);
                let is_exit_game = handle_meta_actions(
                    game_frontend,
                    game_state,
                    game_objects,
                    &mut Some(game_input),
                    meta_action,
                );
                if is_exit_game {
                    game_input.stop_concurrent_input();
                    break;
                }
            }
            Some(PlayerInput::PlayInput(ingame_action)) => {
                debug!("inject ingame action {:#?} to player", ingame_action);
                debug!("attempting to get player {:#?}", game_objects[PLAYER]);
                if let Some(ref mut player) = game_objects[PLAYER] {
                    debug!(
                        "attempting to get prototype {:#?}",
                        player
                            .actions
                            .iter()
                            .find(|a| a.trait_id == ingame_action.trait_id)
                    );
                    if let Some(prototype) = player
                        .actions
                        .iter()
                        .find(|a| a.trait_id == ingame_action.trait_id)
                    {
                        let next_action = Some(get_player_action(ingame_action, prototype));
                        debug!("player action object: {:#?}", next_action);
                        player.set_next_action(next_action);
                    }
                };
            }
            None => {}
        }
    }
}

/// Load an existing savegame and instantiates GameState & Objects
/// from which the game is resumed in the game loop.
pub fn load_game() -> Result<(GameState, GameObjects), Box<dyn Error>> {
    let mut json_save_state = String::new();
    let mut file = File::open(SAVEGAME)?;
    file.read_to_string(&mut json_save_state)?;
    let result = serde_json::from_str::<(GameState, GameObjects)>(&json_save_state)?;
    Ok(result)
}

/// Serialize and store GameState and Objects into a JSON file.
pub fn save_game(game_state: &GameState, objects: &GameObjects) -> Result<(), Box<dyn Error>> {
    let save_data = serde_json::to_string(&(game_state, objects))?;
    let mut file = File::create(SAVEGAME)?;
    file.write_all(save_data.as_bytes())?;
    Ok(())
}
