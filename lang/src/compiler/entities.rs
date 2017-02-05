//! Utility crate containing functions to add new entities to the compiler's script IR

use std::collections::{HashMap, LinkedList};
use std::collections::hash_map::Entry;
use rayon::prelude::*;
use super::Context;
use hct::ast::*;
use vmf::*;

/// Generates the connection nodes correspoding to a script statement
fn stmt_nodes(ctx: &Context, stmt: Statement) -> LinkedList<Connection> {
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
            body.into_par_iter().flat_map(|stmt| stmt_nodes(&ctx, stmt)).collect()
        }
    }
}

/// Adds a block to be executed on map spawn
pub fn add_auto(ctx: &Context, entities: &mut HashMap<String, Entity>, body: LinkedList<Statement>) {
    let mut connections: LinkedList<_> =
        body.into_par_iter()
            .flat_map(|stmt| stmt_nodes(&ctx.auto(), stmt))
            .collect();

    // The empty string can be used without collision risk,
    // as empty targetnames are filtered out when the AST is built
    match entities.entry("".into()) {
        Entry::Vacant(entry) => {
            entry.insert(Entity {
                classname: "logic_auto".into(),
                connections,
                .. Default::default()
            });
        },
        Entry::Occupied(mut entry) => {
            entry.get_mut().connections.append(
                &mut connections
            );
        }
    }
}

/// Adds a block to be executed when a relay is triggered
pub fn add_relay(ctx: &Context, entities: &mut HashMap<String, Entity>, name: String, body: LinkedList<Statement>) {
    entities.insert(name.clone(), Entity {
        classname: "logic_auto".into(),
        targetname: Some(name),
        connections: body.into_par_iter()
            .flat_map(|stmt| stmt_nodes(&ctx.relay(), stmt))
            .collect(),
        .. Default::default()
    });
}

/// Adds a block to be executed when a specified event is called
pub fn add_subscriber(ctx: &Context, subscribers: &mut HashMap<String, LinkedList<Connection>>, path: Path, body: LinkedList<Statement>) {
    let mut list = body.into_par_iter()
        .flat_map(|stmt| stmt_nodes(
            &ctx.subscriber(&path),
            stmt
        ))
        .collect();

    let key = ctx.entity(&path);
    match subscribers.entry(key) {
        Entry::Occupied(ref mut entry) => {
            let mut ent: &mut LinkedList<_> = entry.get_mut();
            ent.append(&mut list);
        },
        Entry::Vacant(entry) => {
            entry.insert(list);
        },
    }
}
