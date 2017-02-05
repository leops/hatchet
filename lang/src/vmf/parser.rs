//! Defines the VMF file parser, re-using the same parser infrastructure as the Hatchet language

use std::collections::LinkedList;
use pest::prelude::*;
use super::tt::*;

impl_rdp! {
    grammar! {
        file = _{ soi ~ block* ~ eoi }
        block = { name ~ ["{"] ~ ( property | block )* ~ block_end }
        property = { string ~ string }

        block_end = { ["}"] }
        name = @{ ( ['A'..'Z'] | ['a'..'z'] | ['0'..'9'] | ["_"] | ["-"] | ["$"] )+ }
        string = @{ ["\""] ~ (!["\""] ~ any)* ~ ["\""] }

        whitespace = _{ [" "] | ["\t"] | ["\u{000C}"] | ["\r"] | ["\n"] }
    }

    process! {
        _block(&self) -> Block {
            (&name: name, mut block: _block()) => {
                block.name = name.into();
                block
            },
            (_: block, head: _block(), mut tail: _block()) => {
                tail.blocks.push_front(head);
                tail
            },
            (_: property, prop: _property(), mut block: _block()) => {
                block.props.push_front(prop);
                block
            },
            (_: block_end) => {
                Default::default()
            }
        }

        _property(&self) -> Property {
            (&key: string, &value: string) => {
                Property {
                    key: key[1..key.len()-1].into(),
                    value: value[1..value.len()-1].into(),
                }
            }
        }
    }
}

// Root VMF files are parsed in a loop as they tend to be long enough
// for recursive parsing to cause a stack overflow
impl<'input, T: Input<'input>> Rdp<T> {
    #[inline]
    pub fn tt(&self) -> LinkedList<Block> {
        let mut blocks = LinkedList::new();

        loop {
            let res = process!(@pattern self
                ({
                    blocks.push_back(block);
                })
                _: block, block: _block()
            );

            if res.is_none() {
                break;
            }
        }

        blocks
    }
}
