pub mod context;
pub mod entities;
pub mod execute;

use std::collections::{HashMap, BTreeMap};
use rayon::prelude::*;

pub use self::context::*;
use hct::ast::*;
use vmf::*;

/// Fold a script onto a map's AST
pub fn apply(script: Script, data: BTreeMap<String, Entity>) -> BTreeMap<String, Entity> {
    // The compiler's intermediate representation of a script,
    // a map of new connections and a map of new entities
    let mut subscribers = HashMap::new();
    let mut entities = HashMap::new();

    // Recursively fill the IR
    execute::fold_items(
        script.items,
        Context::root(&data),
        &mut subscribers,
        &mut entities
    );

    data.into_par_iter()
        // Add the subcriber connections to their respective entities
        .map(|(key, mut ent)| {
            if let Some(subscribers) = subscribers.get(&key) {
                ent.connections.append(&mut subscribers.clone());
            }

            (key.clone(), ent)
        })
        // Append the new entities
        .chain(
            entities.into_par_iter()
        )
        .collect()
}
