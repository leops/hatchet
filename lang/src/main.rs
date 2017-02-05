#![feature(field_init_shorthand)]
#![recursion_limit = "1024"]

#[macro_use]
extern crate pest;
extern crate rayon;
extern crate core;

pub mod hct;
pub mod vmf;
pub mod compiler;

use std::io::{Write, Error, ErrorKind};
use std::fs::{File, create_dir_all};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::thread;
use std::env;

/// Find an instance file relative to a base file by walking up the fs tree
/// Slightly differs from the algorithm used in VBSP, but should work for most cases
/// cf. https://github.com/ValveSoftware/source-sdk-2013/blob/master/mp/src/utils/vbsp/map.cpp#L1904
fn find_instance(mut base: PathBuf, target: PathBuf) -> Option<(PathBuf, PathBuf)> {
    while let Some(dir) = base.clone().parent() {
        let file = dir.join(target.clone());
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

    // Spawn a thread for each instance and leave it to run
    let instances: Vec<thread::JoinHandle<Result<vmf::Instance, Error>>> =
        instances.into_iter()
            .map(|instance| {
                let input = PathBuf::from(input.clone());
                thread::spawn(move || {
                    let target = PathBuf::from(instance.file.clone().unwrap());
                    let (base, input) = find_instance(input, target).unwrap();
                    let result = build(&input)?;
                    Ok(vmf::Instance {
                        file: if result != input {
                            vmf::InstFile::Compiled(
                                format!("{}", result.strip_prefix(&base).unwrap().display())
                            )
                        } else {
                            instance.file
                        },
                        .. instance
                    })
                })
            })
            .collect();

    let vmf_dir = input.parent().ok_or(
        Error::new(ErrorKind::InvalidInput, "Not a directory")
    )?;

    // Progressively fold each script on the map AST
    let mut entities =
        scripts.into_iter()
            .map(|path| hct::parse_file(vmf_dir.join(path)))
            .fold(entities, |map, script| compiler::apply(script, map));

    let hct_dir = vmf_dir.join(".hct");
    create_dir_all(&hct_dir)?;

    let out_path = hct_dir.join(input.file_name().ok_or(
        Error::new(ErrorKind::InvalidInput, "Not a file")
    )?);

    // First, write all the unmodified nodes
    let mut out = File::create(out_path.clone())?;
    for block in nodes {
        write!(&mut out, "{}", block)?;
    }

    // Join the instance threads
    for inst in instances {
        let inst = inst.join().unwrap()?;
        match inst.entity {
            // If the entity was anonymous, write it to the output
            vmf::EntRef::Anon(mut ent) => {
                if let vmf::InstFile::Compiled(new_path) = inst.file {
                    ent.properties.insert("file".into(), new_path);
                }

                write!(&mut out, "{}", ent.into_value())?;
            },
            // If the entity was named, just change the file path as needed
            vmf::EntRef::Named(ref targetname) => {
                let mut ent = entities.get_mut(targetname).unwrap();
                if let vmf::InstFile::Compiled(new_path) = inst.file {
                    ent.properties.insert("file".into(), new_path);
                }
            }
        }
    }

    // Finally, write the entity list to the output
    let iter = entities.into_iter().map(|(_, ent)| ent.into_value());
    for block in iter {
        write!(&mut out, "{}", block)?;
    }

    Ok(out_path)
}

fn main() {
    for argument in env::args().skip(1) {
        let start = Instant::now();

        let vmf_path = Path::new(&argument).with_extension("vmf");
        println!("Transforming file {} ...", vmf_path.display());

        build(&vmf_path).unwrap();

        let duration = start.elapsed();
        println!(
            "Map {} transformed in {}s {}ms",
            vmf_path.display(),
            duration.as_secs(),
            duration.subsec_nanos() / 1000000
        );
    }
}
