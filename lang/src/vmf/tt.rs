//! Defines the VMF file token-tree, a hierarchized version of the token list

use std::fmt::{self, Write, Display, Formatter, Result};
use std::collections::LinkedList;

/// Utility struct for pretty-printing blocks
/// from https://github.com/rust-lang/rust/blob/master/src/libcore/fmt/builders.rs
struct PadAdapter<'a> {
    fmt: &'a mut Write,
    on_newline: bool,
}

impl<'a> PadAdapter<'a> {
    fn new(fmt: &'a mut Write) -> PadAdapter<'a> {
        PadAdapter {
            fmt: fmt,
            on_newline: false,
        }
    }
}

impl<'a> Write for PadAdapter<'a> {
    fn write_str(&mut self, mut s: &str) -> Result {
        while !s.is_empty() {
            if self.on_newline {
                self.fmt.write_str("\t")?;
            }

            let split = match s.find('\n') {
                Some(pos) => {
                    self.on_newline = true;
                    pos + 1
                }
                None => {
                    self.on_newline = false;
                    s.len()
                }
            };
            self.fmt.write_str(&s[..split])?;
            s = &s[split..];
        }

        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
pub struct Property {
    pub key: String,
    pub value: String,
}

impl Display for Property {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        writeln!(fmt, "\"{}\" \"{}\"", self.key, self.value)
    }
}

#[derive(Debug, Default, Clone)]
pub struct Block {
    pub name: String,
    pub props: LinkedList<Property>,
    pub blocks: LinkedList<Block>,
}

impl Display for Block {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        writeln!(fmt, "{}", self.name)?;

        {
            let mut writer = PadAdapter::new(fmt);
            writeln!(writer, "{{")?;

            for prop in self.props.iter() {
                write!(&mut writer, "{}", prop)?;
            }
            for prop in self.blocks.iter() {
                write!(&mut writer, "{}", prop)?;
            }
        }

        writeln!(fmt, "}}")
    }
}
