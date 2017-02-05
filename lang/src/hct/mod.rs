pub mod ast;
pub mod parser;

use pest::prelude::*;
use std::io::prelude::*;
use std::path::Path;
use std::fs::File;

/// Parses a script file, returning its AST
pub fn parse_file<P>(path: P) -> ast::Script where P: AsRef<Path> {
    println!("Parsing script {} ...", path.as_ref().display());

    let mut f = File::open(path).expect("could not open file");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("could not read file");

    let mut parser = parser::Rdp::new(StringInput::new(&s));

    assert!(parser.file());
    assert!(parser.end());

    parser.parse()
}
