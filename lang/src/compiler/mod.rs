pub mod context;
pub mod entities;

use std::collections::{HashMap, BTreeMap, LinkedList, BinaryHeap};
use rayon::prelude::*;

pub use self::context::*;
use self::entities::*;
use hct::ast::*;
use vmf::*;

/// Execute the script items in order, as defined by the implementation of Ord on Item
fn fold_items(mut items: BinaryHeap<Item>, mut ctx: Context, subscribers: &mut HashMap<String, LinkedList<Connection>>, entities: &mut HashMap<String, Entity>) {
    while let Some(item) = items.pop() {
        match item {
            // First register all the bindings in the context
            Item::Binding { name, value } => {
                ctx.set_binding(name, value);
            },

            // Creates all the relays, and register their entities
            Item::Relay { name, body } => {
                ctx.register_ent(name.clone());
                add_relay(&ctx, entities, name, body);
            },

            // Create the auto blocks
            Item::Auto { body } => {
                add_auto(&ctx, entities, body);
            },

            // Add the subscriber connections
            Item::Subscriber { path, body } => {
                add_subscriber(&ctx, subscribers, path, body);
            },

            // Finally, recursively the loops
            Item::Iterator { var, array, items } => {
                let arr = match ctx.binding(&array).expect(&format!("binding {} not found", array)) {
                    &Expression::Array(ref lst) => lst.clone(),
                    exp @ _ => panic!("expression {:?} is not an array", exp),
                };

                for exp in arr {
                    let ctx = ctx.with_binding(var.clone(), exp);
                    fold_items(items.clone(), ctx, subscribers, entities);
                }
            }
        }
    }
}

/// Fold a script onto a map's AST
pub fn apply(script: Script, data: BTreeMap<String, Entity>) -> BTreeMap<String, Entity> {
    // The compiler's intermediate representation of a script,
    // a map of new connections and a map of new entities
    let mut subscribers = HashMap::new();
    let mut entities = HashMap::new();

    // Recursively fill the IR
    fold_items(
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
