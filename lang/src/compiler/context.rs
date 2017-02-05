use std::collections::{HashMap, BTreeMap};
use rayon::prelude::*;
use hct::ast::*;
use vmf::*;

/// Holds the metadata needed to execute a Hatchet script, at any given point
#[derive(Default, Debug)]
pub struct Context<'a> {
    parent: Option<&'a Context<'a>>,
    bindings: HashMap<String, Expression>,
    event: Option<String>,
    delay: Option<f32>,
}

impl<'a> Context<'a> {
    /// Creates the root context from a script using a map's entity list
    pub fn root(entities: &BTreeMap<String, Entity>) -> Context<'a> {
        Context {
            parent: None,
            bindings: entities.par_iter()
                .map(|(key, _)| (
                    key.clone(),
                    Expression::Entity(key.clone())
                ))
                .collect(),
            .. Default::default()
        }
    }

    /// Creates a new context for the execution of an auto block
    pub fn auto(&'a self) -> Context<'a> {
        Context {
            parent: Some(self),
            event: Some(String::from("OnMapSpawn")),
            .. Default::default()
        }
    }

    /// Creates a new context for the execution of a relay block
    pub fn relay(&'a self) -> Context<'a> {
        Context {
            parent: Some(self),
            event: Some(String::from("OnTrigger")),
            .. Default::default()
        }
    }

    /// Creates a new context for the execution of a subscriber block
    pub fn subscriber(&'a self, path: &Path) -> Context<'a> {
        Context {
            parent: Some(self),
            event: Some(self.method(path)),
            .. Default::default()
        }
    }

    /// Creates a new context for the execution of an inner delay block
    pub fn with_delay(&'a self, time: f32) -> Context<'a> {
        Context {
            parent: Some(self),
            delay: Some(time),
            .. Default::default()
        }
    }

    /// Creates a new context with an additional value binding
    pub fn with_binding(&'a self, key: String, value: Expression) -> Context<'a> {
        Context {
            parent: Some(self),
            bindings: {
                let mut res = HashMap::new();
                res.insert(key, value);
                res
            },
            .. Default::default()
        }
    }

    /// Mutate this context to add a new value binding
    pub fn set_binding(&mut self, key: String, value: Expression) {
        self.bindings.insert(key, value);
    }

    /// Get the event currently being executed, if any
    pub fn event(&self) -> Option<String> {
        self.event.clone().or_else(|| self.parent.and_then(|p| p.event()))
    }

    /// Get the delay of the current event, if any
    pub fn delay(&self) -> Option<f32> {
        self.delay.clone().or_else(|| self.parent.and_then(|p| p.delay()))
    }

    /// Get the value of a binding expression
    pub fn binding(&self, key: &String) -> Option<&Expression> {
        match self.bindings.get(key) {
            Some(ref val) => Some(val),
            None => self.parent.and_then(|parent| parent.binding(key))
        }
    }

    /// Mutate this context to add a new entity binding
    pub fn register_ent(&mut self, key: String) {
        self.set_binding(
            key.clone(),
            Expression::Entity(key)
        );
    }

    fn resolve_entity(&self, path: &Path) -> Expression {
        match path {
            &Path::Deref(ref obj, ref prop) => match self.resolve_entity(obj.as_ref()) {
                Expression::Map(ref map) => {
                    map.get(prop)
                        .expect(&format!("key {} not found in {:?}", prop, map)).clone()
                },
                Expression::Reference(ref pat) => self.resolve_entity(pat),
                exp @ Expression::Entity(_) => exp,
                exp @ _ => panic!("unexpected expression {:?}", exp),
            },
            &Path::Instance(ref obj) => self.resolve_entity(obj.as_ref()),
            &Path::Binding(ref name) => {
                match self.binding(name) {
                    Some(val) => val.clone(),
                    None => {
                        if name.starts_with('@') {
                            Expression::Entity(name.clone())
                        } else {
                            panic!("binding {} not found", name)
                        }
                    }
                }
            },
        }
    }

    fn break_path(&self, path: &Path) -> (Option<String>, Option<String>, Option<String>) {
        match path {
            &Path::Deref(ref obj, ref prop) => match self.break_path(obj.as_ref()) {
                (a, Some(b), None) => (a, Some(b), Some(prop.clone())),
                (a, None, None) => (a, Some(prop.clone()), None),
                _ => panic!("invalid path \"{}\"", path),
            },
            &Path::Instance(ref pat) => (Some(self.entity(pat.as_ref())), None, None),
            &Path::Binding(_) => (None, Some(self.entity(path)), None),
        }
    }

    /// Get the entity referenced by a path
    pub fn entity(&self, path: &Path) -> String {
        match self.resolve_entity(path) {
            Expression::Reference(ref pat) => self.entity(pat),
            Expression::Entity(name) => name,
            exp @ _ => panic!("unexpected expression {:?}", exp),
        }
    }

    /// Compute the event string corresponding to a given path
    /// If no event is explicitly called, "Trigger" will be used
    pub fn method(&self, path: &Path) -> String {
        match self.break_path(path) {
            (Some(_), Some(ent), Some(method)) => format!("instance:{};{}", ent, method),
            (None, Some(_), Some(method)) => method,

            (Some(_), Some(ent), None) => format!("instance:{};Trigger", ent),
            (None, Some(_), None) => String::from("Trigger"),

            _ => panic!("invalid path \"{}\"", path),
        }
    }
}
