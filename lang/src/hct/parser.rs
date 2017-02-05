//! Defines the Hatchet parser, using the pest library to tokenize the text input and generate an AST

use std::collections::{LinkedList, HashMap, BinaryHeap};
use pest::prelude::*;
use hct::ast::*;

#[inline]
fn fold_path(path: Option<Path>, name: String) -> Option<Path> {
    Some(match path {
        Some(p) => Path::Deref(Box::new(p), name),
        None => Path::Binding(name),
    })
}

impl_rdp! {
    grammar! {
        file = { soi ~ item* ~ eoi }

        item = { auto | relay | binding | iterator_item | event }
        auto = { ["auto"] ~ ["{"] ~ statement* ~ block_end }
        relay = { ["relay"] ~ name ~ ["{"] ~ statement* ~ block_end }
        iterator_item = { ["for"] ~ name ~ ["in"] ~ name ~ ["{"] ~ item* ~ block_end }
        binding = { ["let"] ~ name ~ ["="] ~ expression }
        event = { path ~ ["{"] ~ statement* ~ block_end }

        statement = { call | delay | iterator_stmt }
        call = { path ~ ["("] ~ (string | number)? ~ [")"] }
        delay = { ["delay"] ~ number ~ ["{"] ~ statement* ~ block_end }
        iterator_stmt = { ["for"] ~ name ~ ["in"] ~ name ~ ["{"] ~ statement* ~ block_end }

        expression = { array | map | path }
        array = { ["["] ~ ( expression ~ ( [","] ~ expression )* ~ [","]? )? ~ array_end }
        map = { ["{"] ~ ( pair ~ ( [","] ~ pair )* ~ [","]? )? ~ block_end }
        pair = { name ~ [":"] ~ expression }

        path = @{ inst_name? ~ ent_name }
        inst_name = @{ name ~ ( ["."] ~ name )* ~ [":"] }
        ent_name = @{ name ~ ( ["."] ~ name )* }

        name = @{ ( ['A'..'Z'] | ['a'..'z'] | ['0'..'9'] | ["_"] | ["-"] | ["$"] | ["@"] )+ }
        number = @{ ['0'..'9']+ ~ ["."] ~ ['0'..'9']+ | ["."] ~ ['0'..'9']+ | ['0'..'9']+ }
        string = @{ ["\""] ~ (!["\""] ~ any)* ~ ["\""] }

        block_end = { ["}"] }
        array_end = { ["]"] }

        whitespace = _{ [" "] | ["\t"] | ["\u{000C}"] | ["\r"] | ["\n"] }
        comment = _{ ["//"] ~ (!(["\r"] | ["\n"]) ~ any)* ~ (["\n"] | ["\r\n"] | ["\r"] | eoi) }
    }

    process! {
        parse(&self) -> Script {
            (_: file, items: _items()) => {
                Script {
                    items: items,
                }
            }
        }

        _items(&self) -> BinaryHeap<Item> {
            (_: item, head: _item(), mut tail: _items()) => {
                // Emit a separate binding node for named entities,
                // to hoist up the declaration in the context
                match head {
                    Item::Relay { ref name, .. } => {
                        tail.push(Item::Binding {
                            name: name.clone(),
                            value: Expression::Entity(
                                name.clone()
                            ),
                        });
                    },

                    _ => {}
                }

                tail.push(head);
                tail
            },
            (_: block_end) => {
                BinaryHeap::new()
            },
            () => {
                BinaryHeap::new()
            }
        }
        _item(&self) -> Item {
            (_: auto, body: _block()) => {
                Item::Auto {
                    body: body,
                }
            },
            (_: relay, &var: name, body: _block()) => {
                Item::Relay {
                    name: var.into(),
                    body: body,
                }
            },
            (_: event, _: path, var: _path(), body: _block()) => {
                Item::Subscriber {
                    path: var,
                    body: body,
                }
            },
            (_: binding, &name: name, _: expression, value: _expression()) => {
                Item::Binding {
                    name: name.into(),
                    value: value,
                }
            },
            (_: iterator_item, &var: name, &array: name, body: _items()) => {
                Item::Iterator {
                    var: var.into(),
                    array: array.into(),
                    body: body,
                }
            }
        }

        _block(&self) -> LinkedList<Statement> {
            (_: statement, head: _statement(), mut tail: _block()) => {
                tail.push_front(head);
                tail
            },
            (_: block_end) => {
                LinkedList::new()
            }
        }

        _statement(&self) -> Statement {
            (_: call, _:path, path: _path(), arg: _literal()) => {
                Statement::Call {
                    path: path,
                    arg: arg,
                }
            },
            (_: delay, &time: number, body: _block()) => {
                Statement::Delay {
                    time: time.parse().expect("invalid number"),
                    body: body,
                }
            },
            (_: iterator_stmt, &var: name, &array: name, body: _block()) => {
                Statement::Iterator {
                    var: var.into(),
                    array: array.into(),
                    body: body,
                }
            }
        }

        _expression(&self) -> Expression {
            (_: array, val: _array()) => {
                Expression::Array(val)
            },
            (_: map, val: _map()) => {
                Expression::Map(val)
            },
            (_: path, val: _path()) => {
                Expression::Reference(val)
            }
        }
        _array(&self) -> LinkedList<Expression> {
            (_: expression, head: _expression(), mut tail: _array()) => {
                tail.push_front(head);
                tail
            },
            (_: array_end) => {
                LinkedList::new()
            }
        }
        _map(&self) -> HashMap<String, Expression> {
            (_:pair, &key: name, _: expression, value: _expression(), mut tail: _map()) => {
                tail.insert(key.into(), value);
                tail
            },
            (_: block_end) => {
                HashMap::new()
            }
        }

        _path(&self) -> Path {
            (_: inst_name, inst: _name_chain(), _: ent_name, pat: _name_chain()) => {
                pat.into_iter()
                    .fold(
                        inst.into_iter()
                            .fold(None, fold_path)
                            .map(|p| Path::Instance(Box::new(p))),
                        fold_path
                    )
                    .expect("empty path")
            },
            (_: ent_name, pat: _name_chain()) => {
                pat.into_iter()
                    .fold(None, fold_path)
                    .expect("empty path")
            }
        }
        _name_chain(&self) -> LinkedList<String> {
            (&head: name, mut tail: _name_chain()) => {
                tail.push_front(head.into());
                tail
            },
            () => {
                LinkedList::new()
            }
        }

        _literal(&self) -> Literal {
            (&num: number) => {
                Literal::Number(num.parse().expect("invalid number"))
            },
            (&string: string) => {
                Literal::String(string[1..string.len()-1].into())
            },
            () => {
                Literal::Void
            }
        }
    }
}
