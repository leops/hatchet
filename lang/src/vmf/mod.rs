pub mod ir;

use std::io::prelude::*;
use std::path::Path;
use std::fs::File;
use vmfparser::*;

pub use vmfparser::ast::*;
pub use vmfparser::parser::*;
pub use self::ir::*;

// Parse a VMF file, returning an IR representation
pub fn parse_file<P>(path: P) -> MapFile where P: AsRef<Path> {
    let mut f = File::open(path).expect("could not open file");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("could not read file");

    let parse_start = timer_start!();
    let ast = parse(&s).unwrap();

    let trans_start = timer_chain!(parse_start, time, "Parsing time: {}", time);
    let res = MapFile::from_ast(ast);

    timer_end!(trans_start, time, "Loading time: {}", time);
    res
}
