//! Utility functions to execute a Hatchet script

use std::collections::{HashMap, LinkedList, BinaryHeap};
use rayon::prelude::*;

use vmf::ir::*;
use hct::ast::*;
use super::context::*;
use super::entities::*;

/// Generates the connection nodes corresponding to a script statement
pub fn stmt_nodes(stmt: Statement, ctx: &Context) -> LinkedList<Connection> {
    match stmt {
        Statement::Call { path, arg } => {
            let mut lst = LinkedList::new();
            lst.push_back(Connection {
                event: ctx.event().expect("not in event scope"),
                entity: ctx.entity(&path),
                method: ctx.method(&path),
                arg: arg,
                delay: ctx.delay().unwrap_or(0.0),
                once: false,
            });
            lst
        },

        Statement::Delay { body, time } => {
            let ctx = ctx.with_delay(time);
            body.into_par_iter().flat_map(|stmt| stmt_nodes(stmt, &ctx)).collect()
        },

        Statement::Iterator { var, array, body } => {
            let arr = match ctx.binding(&array).expect(&format!("binding {} not found", array)) {
                &Expression::Array(ref lst) => lst.clone(),
                exp @ _ => panic!("expression {:?} is not an array", exp),
            };

            arr.into_par_iter()
                .flat_map(|exp| {
                    let ctx = ctx.with_binding(var.clone(), exp);
                    body.par_iter()
                        .flat_map(|stmt| stmt_nodes(stmt.clone(), &ctx))
                        .collect::<LinkedList<_>>()
                })
                .collect()
        },
    }
}

/// Execute the script items in order, as defined by the implementation of Ord on Item
pub fn fold_items(mut items: BinaryHeap<Item>, mut ctx: Context, subscribers: &mut HashMap<String, LinkedList<Connection>>, entities: &mut HashMap<String, Entity>) {
    while let Some(item) = items.pop() {
        match item {
            // First register all the bindings in the context
            Item::Binding { name, value } => {
                ctx.set_binding(name, value);
            },

            // Creates all the relays, and register their entities
            Item::Relay { name, body } => {
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

            // Finally, recursively execute the control nodes
            Item::Iterator { var, array, body } => {
                let arr = match ctx.binding(&array).expect(&format!("binding {} not found", array)) {
                    &Expression::Array(ref lst) => lst.clone(),
                    exp @ _ => panic!("expression {:?} is not an array", exp),
                };

                for exp in arr {
                    let ctx = ctx.with_binding(var.clone(), exp);
                    fold_items(body.clone(), ctx, subscribers, entities);
                }
            },
        }
    }
}
