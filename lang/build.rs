extern crate string_cache_codegen;

use std::env;
use std::path::Path;
use std::fs::OpenOptions;
use std::io::prelude::*;

static NAMES: &'static [&'static str] = &[
    "file", "entity", "connections", "logic_hatchet", "script", "seed", "func_instance",
    "targetname", "classname", "logic_relay", "logic_auto", "x", "r", "pitch", "y", "g", "yaw",
    "z", "b", "roll", "w", "a", "Trigger", "OnMapSpawn", "OnTrigger",
];

static FUNCTIONS: &'static [&'static str] = &[
    "range", "length",
    "exp", "sqrt", "pow", "sin", "cos", "floor", "ceil", "round", "fmuladd",
    "rand", "create", "clone", "remove", "find", "find_class",
    "print", "concat", "to_string", "parse", "get_instance",
    "get_property", "get_sub_property", "set_property", "set_sub_property",
    "create_connection",
];
static GENERICS: &'static [&'static str] = &[
    "vec_len", "vec_get", "eq",
];
static TYPES: &'static [&'static str] = &[
    "f64", "bool", "i64", "Atom", "Entity", "String",
];

fn main() {
    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("atom.rs");
    string_cache_codegen::AtomType::new("atom::Atom", "hct_atom!")
        .atoms(
            NAMES.into_iter()
                .chain(
                    FUNCTIONS.into_iter()
                )
                .map(|n| String::from(*n))
                .chain(
                    GENERICS.into_iter()
                        .flat_map(|n| {
                            TYPES.into_iter()
                                .map(|t| format!("{}.{}", n, t))
                                .collect::<Vec<_>>()
                        })
                )
        )
        .write_to_file(&out_path)
        .unwrap();

    let mut out_file = {
        OpenOptions::new()
            .write(true)
            .append(true)
            .open(&out_path)
            .unwrap()
    };

    writeln!(out_file, "macro_rules! hct_atom_function {{").unwrap();

    for n in FUNCTIONS {
        writeln!(out_file, "((), {0}) => ( hct_atom!(\"{0}\") );", n).unwrap();
    }
    for n in GENERICS {
        for t in TYPES {
            writeln!(out_file, "({1}, {0}) => ( hct_atom!(\"{0}.{1}\") );", n, t).unwrap();
        }
    }

    writeln!(out_file, "}}").unwrap();
}
