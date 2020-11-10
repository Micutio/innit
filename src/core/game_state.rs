use rand::{Rng, RngCore};

use tcod::colors::{self, Color};

use crate::core::game_env::GameEnv;
use crate::core::game_objects::GameObjects;
use crate::core::position::Position;
use crate::entity::action::*;
use crate::entity::control::Controller;
use crate::entity::genetics::GeneLibrary;
use crate::entity::object::Object;
use crate::entity::player::PLAYER;
use crate::ui::game_frontend::{AnimationType, FovMap};
use crate::util::game_rng::{GameRng, RngExtended};

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub enum MsgClass {
    Info,
    Action,
    Alert,
    Story,
}

/// Messages are expressed as colored text.
pub type Messages = Vec<(String, MsgClass)>;

/// The message log can add text from any string collection.
pub trait MessageLog {
    fn add<T: Into<String>>(&mut self, message: T, class: MsgClass);
}

impl MessageLog for Vec<(String, MsgClass)> {
    fn add<T: Into<String>>(&mut self, message: T, class: MsgClass) {
        self.push((message.into(), class));
    }
}

/// Results from processing an objects action for that turn, in ascending rank.
#[derive(PartialEq, Debug)]
pub enum ObjectProcResult {
    NoAction,      // object did not act and is still pondering its turn
    NoFeedback,    // action completed, but requires no visual feedback
    CheckEnterFOV, // check whether the object has entered the player FOV
    Message {
        msg: String,
        class: MsgClass,
        origin: Position,
    }, // display a message, if position of origin is visible to the player
    Animate {
        anim_type: AnimationType,
    }, // play given animation to visualise action
    UpdateFOV,     // action completed, requires updating FOV
    ReRender,      // trigger full re-render of the game world
}

/// The game state struct contains all information necessary to represent the current state of the
/// game, EXCEPT the object vector.
#[derive(Serialize, Deserialize)]
pub struct GameState {
    pub env: GameEnv,
    pub rng: GameRng,
    pub log: Messages,
    pub turn: u128,
    pub dungeon_level: u32,
    pub gene_library: GeneLibrary,
    pub current_obj_index: usize,
    pub current_player_index: usize,
}

impl GameState {
    pub fn new(env: GameEnv, level: u32) -> Self {
        GameState {
            // create the list of game messages and their colours, starts empty
            env,
            rng: GameRng::new_from_u64_seed(rand::thread_rng().next_u64()),
            log: vec![],
            turn: 0,
            dungeon_level: level,
            gene_library: GeneLibrary::new(),
            current_obj_index: 0,
            current_player_index: PLAYER,
        }
    }

    pub fn is_players_turn(&self) -> bool {
        self.current_obj_index == self.current_player_index
    }

    /// Process an object's turn i.e., let it perform as many actions as it has energy for.
    // TODO: Implement energy costs for actions.
    pub fn process_object(
        &mut self,
        objects: &mut GameObjects,
        fov_map: &FovMap,
    ) -> ObjectProcResult {
        let mut process_result = ObjectProcResult::NoAction;
        // unpack object to process its next action
        if let Some(mut active_object) = objects.extract(self.current_obj_index) {
            if let Some(Controller::Player(_)) = active_object.control {
                self.current_player_index = self.current_obj_index;
            }
            trace!(
                "{} | {}'s turn now @energy {}/{}",
                self.current_obj_index,
                active_object.visual.name,
                active_object.processors.energy,
                active_object.processors.energy_storage
            );
            // TURN ACTION PHASE
            // only act if enough energy is available
            if active_object.processors.energy < active_object.processors.energy_storage {
                process_result = ObjectProcResult::NoFeedback;
                active_object.metabolize();
            } else if let Some(next_action) = active_object.get_next_action(self, objects) {
                let energy_cost = next_action.get_energy_cost();

                // perform action
                process_result =
                    self.process_action(fov_map, objects, &mut active_object, next_action);

                // AFTER ACTION PHASE
                if process_result != ObjectProcResult::NoAction {
                    active_object.processors.energy -= energy_cost;

                    if active_object.dna.raw.is_empty() {
                        println!("{} dna is empty!", active_object.visual.name);
                    }
                    if !active_object.dna.raw.is_empty()
                        && self.rng.flip_with_prob(1.0 - active_object.gene_stability)
                    {
                        // mutate the object's genome by randomly flipping a bit
                        let random_gene = self.rng.gen_range(0, active_object.dna.raw.len());
                        let old_gene = active_object.dna.raw[random_gene];
                        debug!(
                            "{} flipping gene: 0b{:08b}",
                            active_object.visual.name, active_object.dna.raw[random_gene]
                        );
                        active_object.dna.raw[random_gene] ^= self.rng.random_bit();
                        debug!(
                            "{}            to: 0b{:08b}",
                            active_object.visual.name, active_object.dna.raw[random_gene]
                        );

                        // apply new genome to object
                        let (sensors, processors, actuators, dna) = self.gene_library.decode_dna(
                            active_object.dna.dna_type,
                            active_object.dna.raw.as_slice(),
                        );
                        active_object.change_genome(sensors, processors, actuators, dna);

                        // TODO: Show mutation effect as diff between old and new genome!
                        if self.current_obj_index == self.current_player_index {
                            self.log.add(
                                format!("A mutation occurred in your genome {}", old_gene),
                                MsgClass::Alert,
                            );
                        } else if let Some(player) = &objects[self.current_player_index] {
                            debug!(
                                "sensing range: {}, dist: {}",
                                player.sensors.sensing_range as f32,
                                player.pos.distance(&active_object.pos)
                            );
                            if player.pos.distance(&active_object.pos)
                                <= player.sensors.sensing_range as f32
                            {
                                // don't record all tiles passing constantly
                                self.log.add(
                                    format!("{} mutated!", active_object.visual.name),
                                    MsgClass::Info,
                                );
                            }
                        }
                    }
                }
            }

            // FINISH TURN PHASE
            if process_result != ObjectProcResult::NoAction {}
            // return object back to objects vector
            objects[self.current_obj_index].replace(active_object);
        } else {
            trace!("no object at index {}", self.current_obj_index);
        }

        // only increase counter if the object has made a move
        if process_result != ObjectProcResult::NoAction {
            self.current_obj_index = (self.current_obj_index + 1) % objects.get_num_objects();
            // also increase turn count if we're back at the player
            if self.current_obj_index == self.current_player_index {
                self.turn += 1;
            }
        }
        process_result
    }

    /// Process an action of the given object.
    fn process_action(
        &mut self,
        fov_map: &FovMap,
        objects: &mut GameObjects,
        actor: &mut Object,
        action: Box<dyn Action>,
    ) -> ObjectProcResult {
        // first execute action, then process result and return
        match action.perform(self, objects, actor) {
            ActionResult::Success { callback } => {
                match callback {
                    ObjectProcResult::CheckEnterFOV => {
                        if self.current_obj_index == self.current_player_index {
                            // if we have the player, then it will surely be in it's own fov
                            ObjectProcResult::UpdateFOV
                        } else if fov_map.is_in_fov(actor.pos.x, actor.pos.y) {
                            // if the acting object is inside the FOV now, trigger a re-render
                            ObjectProcResult::ReRender
                        } else {
                            ObjectProcResult::NoFeedback
                        }
                    }
                    // only play animations if the object is visible to our hero
                    ObjectProcResult::Animate { anim_type } => {
                        if fov_map.is_in_fov(actor.pos.x, actor.pos.y) {
                            ObjectProcResult::Animate { anim_type }
                        } else {
                            ObjectProcResult::NoFeedback
                        }
                    }
                    _ => callback,
                }
            }
            ActionResult::Failure => {
                // how to handle fails?
                ObjectProcResult::NoAction
            }
            ActionResult::Consequence { action } => {
                // if we have a side effect, process it first and then the `main` action
                let _consequence_result =
                    self.process_action(fov_map, objects, actor, action.unwrap());
                // TODO: Think of a way to handle both results of action and consequence.
                // TODO: Extract into function, recursively bubble up results and return the highest
                // priority
                ObjectProcResult::ReRender // use highest priority for now as a dummy
            }
        }
    }
}

// NOTE: All functions below are hot candidates for a rewrite because they might not fit into the
//       new command pattern system.

pub struct Transition {
    pub level: u32,
    pub value: u32,
}

/// Return a value that depends on dungeon level.
/// The table specifies what value occurs after each level, default is 0.
pub fn from_dungeon_level(table: &[Transition], level: u32) -> u32 {
    table
        .iter()
        .rev()
        .find(|transition| level >= transition.level)
        .map_or(0, |transition| transition.value)
}
