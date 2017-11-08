pub mod ast;
pub mod parser;
mod expression;

use std::io::prelude::*;
use std::path::Path;
use std::fs::File;
use synom::*;
use synom::space::*;
use log::LogLevel::Trace;

use logging::*;
use self::ast::Script;
use self::parser::*;

/// Parses a script file, returning its AST
pub fn parse_file<P>(path: P) -> Script where P: AsRef<Path> {
    let mut f = File::open(path).expect("could not open file");
    let mut s = String::new();
    f.read_to_string(&mut s).expect("could not read file");

    let parse_start = timer_start!();

    let res = match script(&s) {
        IResult::Done(rem, script) => {
            if log_enabled!(Trace) {
                trace!("Parser output:");
                print_script(&script).unwrap();
            }

            assert_eq!(skip_whitespace(rem), "");
            script
        },
        IResult::Error => panic!("hatchet parser error"),
    };

    timer_end!(parse_start, time, "Parsing time: {}", time);

    res
}
