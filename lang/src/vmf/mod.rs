pub mod parser;
pub mod ast;
pub mod ir;

use std::io::prelude::*;
use std::path::Path;
use std::fs::File;

use pest::prelude::*;
pub use self::ast::*;
pub use self::ir::*;

// Parse a VMF file, returning an IR representation
pub fn parse_file<P>(path: P) -> MapFile where P: AsRef<Path> {
    let mut f = File::open(path).expect("could not open file");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("could not read file");

    let mut parser = parser::Rdp::new(StringInput::new(&s));

    assert!(parser.file());
    assert!(parser.end());

    MapFile::from_tt(parser.tt())
}
