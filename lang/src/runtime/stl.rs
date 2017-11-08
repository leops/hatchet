use std::collections::HashMap;
use std::ffi::CString;
use std::cell::RefCell;

use rand::Rng;
use rayon::prelude::*;

use atom::Atom;
use vmf::ir::{Connection, Entity};
use compiler::builder::Builder;
use compiler::types::TypeId;
use super::types::*;

declare_externals! {
    intrinsic!(exp = "llvm.exp.f64" (f64) -> f64);
    intrinsic!(sqrt = "llvm.sqrt.f64" (f64) -> f64);
    intrinsic!(pow = "llvm.pow.f64" (f64, f64) -> f64);
    intrinsic!(sin = "llvm.sin.f64" (f64) -> f64);
    intrinsic!(cos = "llvm.cos.f64" (f64) -> f64);
    intrinsic!(floor = "llvm.floor.f64" (f64) -> f64);
    intrinsic!(ceil = "llvm.ceil.f64" (f64) -> f64);
    intrinsic!(round = "llvm.round.f64" (f64) -> f64);
    intrinsic!(fmuladd = "llvm.fmuladd.f64" (f64, f64, f64) -> f64);

    fn rand(context: Context, low: f64, high: f64) -> f64 {
        context.rng.gen_range(low, high)
    }

    fn create(context: Context, name: String, class: String) -> Entity {
        let name = Atom::from(name.clone());
        context.entities.insert(name.clone(), Entity {
            classname: Atom::from(class.clone()),
            targetname: Some(name.clone()),
            .. Default::default()
        });

        name
    }

    fn clone(context: Context, name: Entity) -> Entity {
        let mut ent = {
            let ent = {
                context.entities.get_mut(name)
                    .expect(&format!("entity \"{}\" not found", name))
            };

            ent.clones += 1;
            ent.clone()
        };

        let name = Atom::from(format!("{}_{}", name, ent.clones));
        ent.targetname = Some(name.clone());
        context.entities.insert(name.clone(), ent);

        name
    }

    fn remove(context: Context, name: Entity) {
        context.entities.remove(name)
            .expect(&format!("remove: entity \"{}\" not found", name));

        context.entities.par_iter_mut()
            .for_each(|(_, other)| {
                other.connections = {
                    other.connections
                        .par_iter()
                        .filter(|conn| {
                            conn.entity != *name
                        })
                        .cloned()
                        .collect()
                };
            });
    }

    #[readonly]
    fn find(context: Context, name: String) -> Entity {
        Atom::from(name as &str)
    }

    #[readonly]
    fn find_class(context: Context, class: String) -> (Vec<Entity>) {
        let class = Atom::from(class.clone());
        context.entities.values()
            .filter_map(|entity| {
                if entity.classname == class {
                    entity.targetname
                        .clone()
                        .map(|name| context.arenas.atoms.alloc(name) as *const _)
                } else {
                    None
                }
            })
            .collect()
    }

    #[readonly]
    fn vec_len<T>(array: (Vec<T>)) -> i64 {
        array.len() as _
    }

    #[readonly]
    fn vec_get<T>(array: (Vec<T>), index: i64) -> (ref T) {
        array[index as usize]
    }

    #[readonly]
    fn concat(context: Context, lhs: String, rhs: String) -> String {
        format!("{}{}", lhs, rhs).into()
    }

    #[readonly]
    fn to_string(context: Context, val: f64) -> String {
        val.to_string()
    }

    #[readonly]
    fn parse(val: String) -> f64 {
        val.parse().expect("not a number")
    }

    #[readonly]
    fn eq<(T: Eq)>(lhs: T, rhs: T) -> bool {
        lhs == rhs
    }

    fn print(val: Entity) {
        info!("{:?}", val);
    }

    fn get_property(context: Context, entity: (ref Entity), key: Atom) -> (ref String) {
        entity.properties.get(key).expect("key not found")
    }

    fn get_sub_property(context: Context, value: String, index: i64) -> String {
        value.split_whitespace().nth(index as usize).unwrap().into()
    }

    fn set_property(context: Context, entity: (mut Entity), key: Atom, value: String) {
        entity.properties.insert(key.clone(), value.clone());
    }

    fn set_sub_property(context: Context, entity: (mut Entity), key: Atom, index: i64, value: String) {
        let mut current: Vec<String> = {
            entity.properties.get(key)
                .expect(&format!("property \"{}\" not found", key))
                .split_whitespace()
                .map(|s| s.into())
                .collect()
        };

        current[index as usize] = value.to_string();

        let joined = current.join(" ");
        entity.properties.insert(key.clone(), joined);
    }

    #[readonly]
    fn get_instance(context: Context, ent: Entity, method: Atom) -> Atom {
        Atom::from(format!("instance:{};{}", ent, method))
    }

    fn create_connection(context: Context, from: (mut Entity), event: Atom, entity: Entity, method: Atom, arg: String, delay: f64) {
        from.connections.push(Connection {
            event: event.clone(),
            entity: entity.clone(),
            method: method.clone(),
            arg: arg.to_string(),
            delay,
            once: false,
        });
    }
}
