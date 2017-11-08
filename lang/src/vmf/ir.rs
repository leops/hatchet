//! Defines the VMF intermediate representation, a more opinionated version of the AST hierarchy

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::iter::once;

use rayon::prelude::*;
use synom::IResult;

use atom::Atom;
use hct::parser::number;
use super::parser::string;
use super::ast::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub event: Atom,
    pub entity: Atom,
    pub method: Atom,
    pub arg: String,
    pub delay: f64,
    pub once: bool,
}

impl Connection {
    fn from_value(prop: &Property<Atom>) -> Connection {
        let mut evt: Vec<_> = prop.value.split('\x1b').collect();
        if evt.len() == 1 {
            evt = prop.value.split(',').collect();
        }

        assert_eq!(evt.len(), 5, "invalid event: {:?}", evt);
        Connection {
            event: prop.key.clone(),
            entity: evt[0].into(),
            method: evt[1].into(),
            arg: {
                if let IResult::Done(_, val) = number(evt[2]) {
                    val.to_string()
                } else if let IResult::Done(_, val) = string(evt[2]) {
                    val.into()
                } else {
                    "".into()
                }
            },
            delay: evt[3].parse().expect("invalid number"),
            once: evt[4] == "1"
        }
    }
    pub fn into_value(self) -> Property<Atom> {
        Property {
            key: self.event,
            value: format!(
                "{}\x1b{}\x1b{}\x1b{}\x1b{}",
                self.entity, self.method,
                self.arg, self.delay,
                if self.once { 1 } else { -1 }
            ),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Entity {
    pub classname: Atom,
    pub targetname: Option<Atom>,
    pub properties: HashMap<Atom, String>,
    pub connections: Vec<Connection>,
    pub body: Vec<Block<Atom>>,
    pub clones: u64,
}

impl PartialEq for Entity {
    fn eq(&self, other: &Entity) -> bool {
        self.classname == other.classname &&
        self.targetname == other.targetname &&
        self.properties == other.properties &&
        self.connections == other.connections &&
        self.clones == other.clones
    }
}

impl Entity {
    fn from_value(block: &Block<Atom>) -> Option<Entity> {
        if block.name == hct_atom!("entity") {
            let mut ent = Entity::default();
            for prop in &block.props {
                match prop.key.as_ref() {
                    "classname" => {
                        ent.classname = Atom::from(prop.value.clone());
                    },
                    "targetname" if !prop.value.is_empty() => {
                        ent.targetname = Some(Atom::from(prop.value.clone()));
                    },
                    _ => {
                        ent.properties.insert(prop.key.clone(), prop.value.clone());
                    }
                }
            }
            for block in &block.blocks {
                if block.name == hct_atom!("connections") {
                    for prop in &block.props {
                        ent.connections.push(
                            Connection::from_value(prop)
                        );
                    }
                } else {
                    ent.body.push(block.clone());
                }
            }

            Some(ent)
        } else {
            None
        }
    }
    pub fn into_value(self) -> Block<Atom> {
        Block {
            name: hct_atom!("entity"),
            props: {
                self.properties.into_iter()
                    .map(|(key, value)| Property {
                        key, value
                    })
                    .chain(
                        self.targetname.into_iter()
                            .map(|name| Property {
                                key: hct_atom!("targetname"),
                                value: name.to_string(),
                            })
                    )
                    .chain(
                        once(
                            Property {
                                key: hct_atom!("classname"),
                                value: self.classname.to_string(),
                            }
                        )
                    )
                    .collect()
            },
            blocks: {
                self.body.into_iter()
                    .chain(
                        once(
                            Block {
                                name: hct_atom!("connections"),
                                props: {
                                    self.connections.into_iter()
                                        .map(|conn| conn.into_value())
                                        .collect()
                                },
                                .. Default::default()
                            }
                        )
                    )
                    .collect()
            },
        }
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum InstFile {
    Original(String),
    Compiled(String),
}

impl InstFile {
    pub fn unwrap(self) -> String {
        match self {
            InstFile::Original(val) |
            InstFile::Compiled(val) => val,
        }
    }
}

#[derive(Debug)]
pub enum EntRef {
    Named(Atom),
    Anon(Entity),
}

#[derive(Debug)]
pub struct Instance {
    pub file: InstFile,
    pub entity: EntRef,
}

impl PartialEq for Instance {
    fn eq(&self, other: &Instance) -> bool {
        self.file.eq(&other.file)
    }
}

impl Eq for Instance {}

impl PartialOrd for Instance {
    fn partial_cmp(&self, other: &Instance) -> Option<::std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Instance {
    fn cmp(&self, other: &Instance) -> ::std::cmp::Ordering {
        self.file.cmp(&other.file)
    }
}

impl Hash for Instance {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.file.hash(state);
    }
}

#[derive(Clone, Debug)]
pub struct Script {
    pub script: String,
    pub seed: usize,
}

#[derive(Debug, Default)]
pub struct MapFile {
    pub nodes: Vec<Block<Atom>>,
    pub entities: HashMap<Atom, Entity>,
    pub scripts: Vec<Script>,
    pub instances: HashSet<Instance>,
}

impl MapFile {
    pub fn from_ast(tree: Vec<Block<Atom>>) -> MapFile {
        tree.into_par_iter()
            .fold(
                MapFile::default,
                |mut infos, block| {
                    match Entity::from_value(&block) {
                        // Script entities are removed from the AST
                        Some(Entity { ref classname, ref properties, .. }) if *classname == hct_atom!("logic_hatchet") => {
                            infos.scripts.push(Script {
                                script: properties.get(&hct_atom!("script"))
                                    .expect("missing script in logic_hatchet").clone(),
                                seed: properties.get(&hct_atom!("seed"))
                                    .map(|seed| {
                                        seed.parse::<usize>().expect("seed is not a valid integer")
                                    })
                                    .unwrap_or_else(|| {
                                        warn!("logic_hatchet with no seed, using 0");
                                        0
                                    }),
                            });
                        },

                        // Instances are placed in a separate list,
                        // used to spawn the sub-compilation threads
                        Some(ref ent) if ent.classname == hct_atom!("func_instance") => {
                            infos.instances.insert(Instance {
                                file: InstFile::Original(
                                    ent.properties.get(&hct_atom!("file"))
                                        .expect("missing file property in func_instance").clone()
                                ),
                                entity: if let Some(ref targetname) = ent.targetname {
                                    infos.entities.insert(targetname.clone(), ent.clone());
                                    EntRef::Named(targetname.clone())
                                } else {
                                    EntRef::Anon(ent.clone())
                                },
                            });
                        },

                        Some(ref ent @ Entity { targetname: Some(_), .. }) => {
                            infos.entities.insert(
                                ent.targetname.clone().unwrap(),
                                ent.clone(),
                            );
                        },

                        _ => {
                            infos.nodes.push(block);
                        },
                    }

                    infos
                }
            )
            .reduce(
                MapFile::default,
                MapFile::merge
            )
    }
    fn merge(a: MapFile, mut b: MapFile) -> MapFile {
        MapFile {
            nodes: {
                let mut lst = a.nodes;
                lst.append(&mut b.nodes);
                lst
            },
            entities: {
                let mut map = a.entities;
                map.extend(b.entities);
                map
            },
            scripts: {
                let mut lst = a.scripts;
                lst.append(&mut b.scripts);
                lst
            },
            instances: {
                let mut lst = a.instances;
                lst.extend(b.instances);
                lst
            },
        }
    }
}
