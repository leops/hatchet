use std::collections::VecDeque;
use std::borrow::Borrow;

use hct::ast::Path;
use super::builder::*;
use super::function::*;
use super::scope::Scope;
use super::types::*;
use atom::*;

pub fn resolve_path<'a, P: Borrow<Path>>(path: P, scope: &Scope<'a>, builder: &mut Builder) -> ValueRef {
    let path = path.borrow();
    match *path {
        Path::Deref(ref obj, ref prop) => {
            let res = resolve_path(obj.borrow(), scope, builder);
            if res.ty == TypeId::Entity {
                res
            } else if let TypeId::Object { ref items } = res.ty {
                let idx = {
                    items.binary_search_by(|&(ref item, _)| item.cmp(prop))
                        .expect(&format!("key \"{}\" not found in object", prop))
                };

                let (_, ref ty) = items[idx];
                let zero = builder.build_const_i32(0i32);
                let idx = builder.build_const_i32(idx as i32);
                let gep = builder.build_in_bounds_gep(
                    &res,
                    &[
                        zero,
                        idx,
                    ],
                );

                let res = builder.build_load(&gep);
                ValueRef { ty: ty.clone(), ptr: res.ptr }
            } else {
                panic!("trying to deref a non-map value {}", *path)
            }
        },
        Path::Instance(ref obj) => resolve_path(obj.borrow(), scope, builder),
        Path::Binding(ref name) => {
            match scope.binding(builder, name) {
                Some(val) => val,
                None => if name.starts_with('@') {
                    builder.build_const_entity(name)
                } else {
                    panic!("entity {} not found", *path)
                }
            }
        },
    }
}

type Trigger<'a> = (Option<ValueRef>, Option<ValueRef>, Option<Atom>);

fn break_trigger<'a>(path: Path, scope: &Scope<'a>, builder: &mut Builder) -> Trigger<'a> {
    match path {
        Path::Deref(obj, prop) => match break_trigger(*obj, scope, builder) {
            (a, Some(b), None) => (a, Some(b), Some(prop)),
            (a, None, None) => (a, Some(builder.build_const_entity(prop)), None),
            path => panic!("invalid path {:?}", path),
        },
        Path::Instance(pat) => (Some(resolve_path(pat, scope, builder)), None, None),
        Path::Binding(_) => (None, Some(resolve_path(&path, scope, builder)), None),
    }
}

/// Get the entity referenced by a path and its associated event string
/// If no event is explicitly called, "Trigger" will be used
pub fn event<'a>(path: Path, scope: &Scope<'a>, builder: &mut Builder) -> (ValueRef, ValueRef) {
    match break_trigger(path, scope, builder) {
        (Some(inst), Some(ent), Some(method)) => {
            let method = builder.build_const_atom(method);
            let method = call_stl(
                builder,
                hct_atom!("get_instance"),
                vec![ &ent, &method ],
            );
            (inst, method)
        },

        (None, Some(ent), Some(method)) => (
            ent,
            builder.build_const_atom(method),
        ),

        (Some(inst), Some(ent), None) => {
            let method = builder.build_const_atom(hct_atom!("Trigger"));
            let method = call_stl(
                builder,
                hct_atom!("get_instance"),
                vec![ &ent, &method ],
            );
            (inst, method)
        },

        (None, Some(ent), None) => (
            ent,
            builder.build_const_atom(hct_atom!("Trigger")),
        ),

        path => panic!("invalid path {:?}", path),
    }
}

/// Recursively transforms a Path to a list
pub fn unwind_path(path: Path) -> VecDeque<Atom> {
    match path {
        Path::Deref(obj, prop) => {
            let mut res = unwind_path(*obj);
            res.push_back(prop);
            res
        },
        Path::Binding(name) => {
            let mut res = VecDeque::new();
            res.push_back(name);
            res
        },
        Path::Instance(_) => unimplemented!(),
    }
}
