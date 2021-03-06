use crate::entity::{action::Action, object::Object};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Inventory {
    /// A list of items contained in this inventory.
    pub items: Vec<Object>,

    /// A list of actions pertaining this inventory, mostly dropping items.
    pub inv_actions: Vec<Box<dyn Action>>,
}

impl Inventory {
    pub fn new() -> Self {
        Inventory {
            items: Vec::new(),
            inv_actions: Vec::new(),
        }
    }
}
