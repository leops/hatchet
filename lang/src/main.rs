#![feature(box_syntax, box_patterns, splice, specialization)]
#![cfg_attr(feature="clippy", feature(plugin))]
#![cfg_attr(feature="clippy", plugin(clippy))]
// The atom macro generates huge u64 literals
#![cfg_attr(feature="clippy", allow(unreadable_literal))]

extern crate rayon;
extern crate rand;
extern crate llvm_sys;
extern crate libc;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate simplelog;
extern crate term;
extern crate string_cache;
extern crate typed_arena;
#[macro_use]
extern crate synom;
extern crate vmfparser;
extern crate diff;
extern crate unreachable;
extern crate either;
#[macro_use]
extern crate lazy_static;

#[macro_use]
pub mod atom {
    include!(concat!(env!("OUT_DIR"), "/atom.rs"));
}

#[macro_use]
pub mod logging;
pub mod hct;
pub mod vmf;
pub mod compiler;
pub mod runtime;

use std::io::{Write, Error, ErrorKind};
use std::fs::{File, create_dir_all};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::env;

use rayon::prelude::*;
use clap::{App, Arg};
use log::LogLevel::Debug;
use simplelog::{SimpleLogger, TermLogger, LogLevelFilter};
use logging::*;

/// Find an instance file relative to a base file by walking up the fs tree
/// Slightly differs from the algorithm used in VBSP, but should work for most cases
/// cf. https://github.com/ValveSoftware/source-sdk-2013/blob/master/mp/src/utils/vbsp/map.cpp#L1904
fn find_instance<P: Copy + AsRef<Path>>(mut base: PathBuf, target: P) -> Option<(PathBuf, PathBuf)> {
    while let Some(dir) = base.clone().parent() {
        let file = dir.join(target);
        if file.exists() {
            return Some((base, file));
        } else {
            base = PathBuf::from(dir);
        }
    }

    None
}

/// Main compiler driver function
/// Runs the build for a map file, recursively spawning threads for all instances
/// Returns the newly created file if one or more script was applied to it,
/// or the original unmodified file otherwise
fn build(input: &PathBuf) -> Result<PathBuf, Error> {
    let vmf::MapFile {
        nodes, entities,
        scripts, instances
    } = vmf::parse_file(input);

    if scripts.is_empty() {
        return Ok(input.clone());
    }

    let vmf_dir = input.parent().ok_or_else(
        || Error::new(ErrorKind::InvalidInput, "Not a directory")
    )?;

    // Progressively fold each script on the map AST
    let entities = Arc::new(Mutex::new(
        scripts.into_iter()
            .map(|ent| {
                let ast = hct::parse_file(vmf_dir.join(&ent.script));
                (ent, ast)
            })
            .fold(entities, |map, (ent, ast)| {
                compiler::apply(ent, ast, map)
            })
    ));

    let data = {
        // Unmodified nodes
        nodes.into_par_iter()
            .map(|block| block.to_string())
            .chain(
                // Instance sub-process
                instances.into_par_iter()
                    .filter_map(|instance| {
                        let target = instance.file.clone().unwrap();
                        let (base, input) = find_instance(input.clone(), &target).unwrap();
                        let result = build(&input).unwrap();
                        let file = if result != input {
                            vmf::InstFile::Compiled(
                                result.strip_prefix(&base).unwrap().display().to_string()
                            )
                        } else {
                            instance.file
                        };

                        match instance.entity {
                            // If the entity was anonymous, write it to the output
                            vmf::EntRef::Anon(mut ent) => {
                                if let vmf::InstFile::Compiled(new_path) = file {
                                    ent.properties.insert(hct_atom!("file"), new_path);
                                }

                                Some(ent.into_value().to_string())
                            },
                            // If the entity was named, just change the file path as needed
                            vmf::EntRef::Named(ref targetname) => {
                                let mut entities = entities.lock().unwrap();
                                let ent = entities.get_mut(targetname).unwrap();
                                if let vmf::InstFile::Compiled(new_path) = file {
                                    ent.properties.insert(hct_atom!("file"), new_path);
                                }

                                None
                            }
                        }
                    })
            )
            .reduce(
                String::new,
                |a, b| a + &b,
            )
    };

    let entities = {
        let entities = Arc::try_unwrap(entities).unwrap();
        let entities = entities.into_inner().unwrap();
        entities.into_par_iter()
            .map(|(_, ent)| ent.into_value().to_string())
            .reduce(
                String::new,
                |a, b| a + &b,
            )
    };

    let hct_dir = vmf_dir.join(".hct");
    create_dir_all(&hct_dir)?;

    let out_path = hct_dir.join(input.file_name().ok_or_else(
        || Error::new(ErrorKind::InvalidInput, "Not a file")
    )?);

    // First, write all the unmodified nodes
    let mut out = File::create(out_path.clone())?;
    write!(&mut out, "{}", data)?;
    write!(&mut out, "{}", entities)?;

    Ok(out_path)
}

fn main() {
    let args = {
        App::new("Hatchet Compiler")
            .version(crate_version!())
            .author(crate_authors!("\n"))
            .about(crate_description!())
            .arg(
                Arg::with_name("INPUT")
                    .help("Input files to process")
                    .required(true)
                    .multiple(true)
            )
            .arg(
                Arg::with_name("d")
                    .short("d")
                    .multiple(true)
                    .help("Show debug informations")
            )
            .arg(
                Arg::with_name("trace")
                    .short("t")
                    .help("Show backtrace in case of crash")
            )
            .get_matches()
    };

    if args.is_present("trace") {
        env::set_var("RUST_BACKTRACE", "1");
    }

    let log_level = match args.occurrences_of("d") {
        0 => LogLevelFilter::Info,
        1 => LogLevelFilter::Debug,
        _ => LogLevelFilter::Trace,
    };
    match TermLogger::init(log_level, Default::default()) {
        Ok(_) => {},
        Err(_) => {
            SimpleLogger::init(log_level, Default::default())
                .expect("could not initialize logger")
        }
    }

    for argument in args.values_of("INPUT").unwrap() {
        let vmf_path = Path::new(&argument).with_extension("vmf");
        let start = if log_enabled!(Debug) {
            Some(Instant::now())
        } else {
            info!("Transforming file {} ...", path_fmt(&vmf_path));
            None
        };

        build(&vmf_path).unwrap();

        if let Some(start) = start {
            let duration = start.elapsed();
            debug!(
                "Map {} transformed in {}",
                path_fmt(vmf_path),
                duration_fmt(duration),
            );
        } else {
            info!("Finished transforming file {}", path_fmt(vmf_path));
        }
    }
}
