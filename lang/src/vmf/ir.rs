//! Defines the VMF intermediate representation, a more opinionated version of the AST hierarchy

use std::collections::{LinkedList, BTreeMap, BTreeSet};
use pest::prelude::*;
use rayon::prelude::*;
use hct::ast::Literal;
use hct::parser;
use super::ast::*;

#[derive(Debug, Clone)]
pub struct Connection {
    pub event: String,
    pub entity: String,
    pub method: String,
    pub arg: Literal,
    pub delay: f32,
    pub once: bool,
}

impl Connection {
    fn from_value(prop: &Property) -> Connection {
        let evt: Vec<_> = prop.value.split('\x1b').collect();
        Connection {
            event: prop.key.clone(),
            entity: evt[0].into(),
            method: evt[1].into(),
            arg: {
                let mut parser = parser::Rdp::new(
                    StringInput::new(evt[2])
                );

                if parser.string() || parser.number() {
                    assert!(parser.end());
                    parser._literal()
                } else {
                    Literal::Void
                }
            },
            delay: evt[3].parse().expect("invalid number"),
            once: evt[4] == "1"
        }
    }

    pub fn into_value(self) -> Property {
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

#[derive(Debug, Default, Clone)]
pub struct Entity {
    pub classname: String,
    pub targetname: Option<String>,
    pub properties: BTreeMap<String, String>,
    pub connections: LinkedList<Connection>,
    pub body: LinkedList<Block>,
}

impl Entity {
    fn from_value(block: &Block) -> Option<Entity> {
        if block.name == "entity" {
            let mut ent = Entity::default();
            for prop in block.props.iter() {
                match prop.key.as_ref() {
                    "classname" => {
                        ent.classname = prop.value.clone();
                    },
                    "targetname" if !prop.value.is_empty() => {
                        ent.targetname = Some(prop.value.clone());
                    },
                    _ => {
                        ent.properties.insert(prop.key.clone(), prop.value.clone());
                    }
                }
            }
            for block in block.blocks.iter() {
                if block.name == "connections" {
                    for prop in block.props.iter() {
                        ent.connections.push_back(
                            Connection::from_value(prop)
                        );
                    }
                } else {
                    ent.body.push_back(block.clone());
                }
            }

            Some(ent)
        } else {
            None
        }
    }

    pub fn into_value(self) -> Block {
        Block {
            name: "entity".into(),
            props: {
                self.properties.into_iter()
                    .map(|(key, value)| Property {
                        key, value
                    })
                    .chain(
                        self.targetname.into_iter()
                            .map(|name| Property {
                                key: "targetname".into(),
                                value: name,
                            })
                    )
                    .chain(
                        ::std::iter::once(
                            Property {
                                key: "classname".into(),
                                value: self.classname
                            }
                        )
                    )
                    .collect()
            },
            blocks: {
                self.body.into_iter()
                    .chain(
                        ::std::iter::once(
                            Block {
                                name: "connections".into(),
                                props: self.connections.into_iter()
                                    .map(|conn| conn.into_value())
                                    .collect(),
                                .. Default::default()
                            }
                        )
                    )
                    .collect()
            },
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
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
    Named(String),
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

#[derive(Debug, Default)]
pub struct MapFile {
    pub nodes: LinkedList<Block>,
    // HashMap doesn't support merging
    pub entities: BTreeMap<String, Entity>,
    pub scripts: LinkedList<String>,
    pub instances: BTreeSet<Instance>,
}

impl MapFile {
    pub fn from_tt(tt: LinkedList<Block>) -> MapFile {
        tt.into_par_iter()
            .fold(
                MapFile::default,
                |mut infos, block| {
                    if let Some(ent) = Entity::from_value(&block) {
                        match ent.classname.as_ref() {
                            // Script entities are removed from the AST
                            "logic_hatchet" => {
                                infos.scripts.push_back(
                                    ent.properties.get("script".into())
                                        .expect("missing script property in logic_hatchet").clone()
                                );
                            },
                            _ => {
                                // Instances are placed in a separate list,
                                // used to spawn the sub-compilation threads
                                if ent.classname == "func_instance" {
                                    infos.instances.insert(Instance {
                                        file: InstFile::Original(
                                            ent.properties.get("file".into())
                                                .expect("missing file property in func_instance").clone()
                                        ),
                                        entity: if let Some(ref targetname) = ent.targetname {
                                            infos.entities.insert(targetname.clone(), ent.clone());
                                            EntRef::Named(targetname.clone())
                                        } else {
                                            EntRef::Anon(ent.clone())
                                        },
                                    });
                                } else if let Some(ref targetname) = ent.targetname {
                                    infos.entities.insert(targetname.clone(), ent.clone());
                                } else {
                                    infos.nodes.push_back(block);
                                }
                            }
                        }
                    } else {
                        infos.nodes.push_back(block);
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
                map.append(&mut b.entities);
                map
            },
            scripts: {
                let mut lst = a.scripts;
                lst.append(&mut b.scripts);
                lst
            },
            instances: {
                let mut lst = a.instances;
                lst.append(&mut b.instances);
                lst
            },
        }
    }
}
