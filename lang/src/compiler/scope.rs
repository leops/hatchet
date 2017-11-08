use std::collections::HashMap;

use super::builder::*;
use super::types::*;
use atom::*;

/// Represent a scope (group of bindings in a block) in a script
pub struct Scope<'a> {
    parent: Option<&'a Scope<'a>>,
    bindings: HashMap<Atom, (bool, ValueRef)>,

    event: Option<(ValueRef, ValueRef)>,
    delay: Option<ValueRef>,
}

impl<'a> Scope<'a> {
    /// Creates the root scope for a script using a map's entity list
    pub fn root(builder: &mut Builder) -> Scope<'a> {
        let keys = builder.get_entities();
        Scope {
            parent: None,
            bindings: {
                keys.into_iter()
                    .map(|k| {
                        let val = builder.build_const_entity(&k);
                        (k, (true, val))
                    })
                    .collect()
            },

            event: None,
            delay: None,
        }
    }

    /// Create a new child scope
    pub fn fork(&'a self) -> Scope<'a> {
        Scope {
            parent: Some(self),
            bindings: Default::default(),

            event: None,
            delay: None,
        }
    }

    fn store_binding(builder: &mut Builder, value: &ValueRef) -> (bool, ValueRef) {
        let ptr = builder.build_alloca(&value.ty);
        builder.build_store(value, &ptr);
        (false, ptr)
    }

    /// Creates a new scope with an additional value binding
    pub fn with_binding(&'a self, builder: &mut Builder, key: Atom, value: &ValueRef) -> Scope<'a> {
        Scope {
            parent: Some(self),
            bindings: {
                let mut res = HashMap::new();
                let value = Scope::store_binding(builder, value);
                res.insert(key, value);
                res
            },

            event: None,
            delay: None,
        }
    }

    /// Mutate this scope to add a new value binding
    pub fn set_binding(&mut self, builder: &mut Builder, key: Atom, value: &ValueRef) {
        let value = Scope::store_binding(builder, value);
        self.bindings.insert(key, value);
    }

    fn binding_ptr(&self, key: &Atom) -> Option<(bool, ValueRef)> {
        self.bindings.get(key).cloned()
            .or_else(|| {
                self.parent.and_then(|parent| parent.binding_ptr(key))
            })
    }

    /// Get the value of a binding
    pub fn binding(&self, builder: &mut Builder, key: &Atom) -> Option<ValueRef> {
        self.binding_ptr(key)
            .map(|(is_const, value)| {
                if is_const {
                    value
                } else {
                    builder.build_load(&value)
                }
            })
    }

    /// Get the mutable address of a binding
    pub fn binding_mut(&self, key: &Atom) -> Option<ValueRef> {
        self.binding_ptr(key)
            .and_then(|(is_const, value)| {
                if is_const {
                    None
                } else {
                    Some(value)
                }
            })
    }

    /// Get the event of this scope
    pub fn event(&self) -> Option<(ValueRef, ValueRef)> {
        self.event.clone().or_else(|| {
            self.parent.and_then(|parent| parent.event())
        })
    }

    /// Get the delay of this scope
    pub fn delay(&self) -> Option<ValueRef> {
        self.delay.clone().or_else(|| {
            self.parent.and_then(|parent| parent.delay())
        })
    }

    /// Creates a new scope for the execution of an auto block
    pub fn auto(&'a self, builder: &mut Builder) -> Scope<'a> {
        Scope {
            parent: Some(self),
            bindings: Default::default(),

            event: Some((
                builder.build_const_entity(hct_atom!("")),
                builder.build_const_atom(hct_atom!("OnMapSpawn")),
            )),
            delay: None,
        }
    }

    /// Creates a new scope for the execution of a relay block
    pub fn relay(&'a self, builder: &mut Builder, ent: &Atom) -> Scope<'a> {
        Scope {
            parent: Some(self),
            bindings: Default::default(),

            event: Some((
                builder.build_const_entity(ent),
                builder.build_const_atom(hct_atom!("OnTrigger")),
            )),
            delay: None,
        }
    }

    /// Creates a new scope for the execution of a subscriber block
    pub fn subscriber(&'a self, entity: ValueRef, method: ValueRef) -> Scope<'a> {
        if let Some((ref self_entity, ref self_method)) = self.event {
            warn!(
                "Nested subscriber blocks: {:?}.{:?} -> {:?}.{:?}",
                self_entity, self_method,
                entity, method,
            );
        }

        Scope {
            parent: Some(self),
            bindings: Default::default(),

            event: Some((entity, method)),
            delay: None,
        }
    }

    /// Creates a new scope for the execution of an inner delay block
    pub fn with_delay(&'a self, builder: &mut Builder, time: ValueRef) -> Scope<'a> {
        assert_eq!(time.ty, TypeId::f64, "Delay is not a number");

        Scope {
            parent: Some(self),
            bindings: Default::default(),

            event: None,
            delay: Some(
                self.delay.clone()
                    .map_or(
                        time.clone(),
                        |val| builder.build_fadd(&val, &time),
                    )
            ),
        }
    }
}
