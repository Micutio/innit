//! Module Ai
//!
//! Structures and methods for constructing the game ai.

// internal imports

use crate::core::game_objects::GameObjects;
use crate::entity::action::{Action, PassAction, Target, TargetCategory};
use crate::entity::object::Object;
use crate::util::game_rng::GameRng;

use crate::entity::control::Ai;
use rand::seq::{IteratorRandom, SliceRandom};
use std::fmt::Debug;

#[derive(Debug, Serialize, Deserialize)]
pub struct PassiveAi;

impl PassiveAi {
    pub fn new() -> Self {
        PassiveAi {}
    }
}

#[typetag::serde]
impl Ai for PassiveAi {
    fn act(
        &self,
        _object: &mut Object,
        _game_objects: &mut GameObjects,
        _game_rng: &mut GameRng,
    ) -> Box<dyn Action> {
        Box::new(PassAction)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RandomAi;

impl RandomAi {
    pub fn new() -> Self {
        RandomAi {}
    }
}

#[typetag::serde]
impl Ai for RandomAi {
    fn act(
        &self,
        object: &mut Object,
        game_objects: &mut GameObjects,
        game_rng: &mut GameRng,
    ) -> Box<dyn Action> {
        // If the object doesn't have any action, return a pass.
        if object.actuators.actions.is_empty()
            && object.processors.actions.is_empty()
            && object.sensors.actions.is_empty()
        {
            return Box::new(PassAction);
        }

        // Get a list of possible targets, blocking and non-blocking, and search only for actions
        // that can be used with these targets.
        let adjacent_targets: Vec<&Object> = game_objects
            .get_vector()
            .iter()
            .flatten()
            .filter(|obj| {
                object.pos.is_adjacent(&obj.pos)
                    && (obj.physics.is_blocking || !game_objects.is_pos_occupied(&obj.pos))
            })
            // .filter_map(|o| o.as_ref())
            .collect();

        println!("adjacent target count: {:?}", &adjacent_targets.len());

        let mut valid_targets = vec![
            TargetCategory::None,
            TargetCategory::Any,
            TargetCategory::EmptyObject,
            TargetCategory::BlockingObject,
        ];

        // options:
        // a) targets empty => only None a.k.a self
        if adjacent_targets.is_empty() {
            valid_targets.retain(|t| *t == TargetCategory::None);
        }

        // b) no empty targets available
        if adjacent_targets
            .iter()
            .filter(|t| !t.physics.is_blocking)
            .count()
            == 0
        {
            valid_targets.retain(|t| *t != TargetCategory::EmptyObject)
        }

        // d) no blocking targets available => remove blocking from selection
        if adjacent_targets
            .iter()
            .filter(|t| t.physics.is_blocking)
            .count()
            == 0
        {
            valid_targets.retain(|t| *t != TargetCategory::BlockingObject);
        }

        // dbg!("valid targets: {:?}", &valid_targets);

        // find an action that matches one of the available target categories
        let possible_actions: Vec<&Box<dyn Action>> = object
            .actuators
            .actions
            .iter()
            .chain(object.processors.actions.iter())
            .chain(object.sensors.actions.iter())
            .filter(|a| valid_targets.contains(&(*a).get_target_category()))
            .collect();

        if let Some(a) = possible_actions.choose(game_rng) {
            let mut boxed_action = a.clone_action();
            match boxed_action.get_target_category() {
                TargetCategory::None => boxed_action.set_target(Target::Center),
                TargetCategory::BlockingObject => {
                    if let Some(target_obj) = adjacent_targets
                        .iter()
                        .filter(|at| at.physics.is_blocking)
                        .choose(game_rng)
                    {
                        boxed_action.set_target(Target::from_pos(&object.pos, &target_obj.pos))
                    }
                }
                TargetCategory::EmptyObject => {
                    if let Some(target_obj) = adjacent_targets
                        .iter()
                        .filter(|at| !at.physics.is_blocking)
                        .choose(game_rng)
                    {
                        boxed_action.set_target(Target::from_pos(&object.pos, &target_obj.pos))
                    }
                }
                TargetCategory::Any => {
                    if let Some(target_obj) = adjacent_targets.choose(game_rng) {
                        boxed_action.set_target(Target::from_pos(&object.pos, &target_obj.pos))
                    }
                }
            }
            boxed_action
        } else {
            Box::new(PassAction)
        }
    }
}
