//! Defines the Hatchet language abstract syntax tree

use std::fmt::{self, Display, Formatter, Error};
use std::collections::{LinkedList, HashMap, BinaryHeap};
use std::cmp::Ordering;

/// Main script entry point
#[derive(Debug, Clone)]
pub struct Script {
    pub items: BinaryHeap<Item>,
}

/// Root script value definitions
#[derive(Debug, Clone)]
pub enum Item {
    Binding {
        name: String,
        value: Expression,
    },

    Relay {
        name: String,
        body: LinkedList<Statement>,
    },

    Auto {
        body: LinkedList<Statement>,
    },

    Subscriber {
        path: Path,
        body: LinkedList<Statement>,
    },

    Iterator {
        var: String,
        array: String,
        body: BinaryHeap<Item>,
    },
}

// As Items need to be executed in a specific order,
// they are pre-sorted in binary heaps during the AST build
impl Ord for Item {
    fn cmp(&self, other: &Item) -> Ordering {
        if self.eq(other) {
            return Ordering::Equal;
        }

        match (self, other) {
            (&Item::Binding { .. }, _) => Ordering::Greater,
            (_, &Item::Binding { .. }) => Ordering::Less,

            (&Item::Relay { .. }, _) => Ordering::Greater,
            (_, &Item::Relay { .. }) => Ordering::Less,

            (&Item::Auto { .. }, _) => Ordering::Greater,
            (_, &Item::Auto { .. }) => Ordering::Less,

            (&Item::Subscriber { .. }, _) => Ordering::Greater,
            (_, &Item::Subscriber { .. }) => Ordering::Less,

            (&Item::Iterator { .. }, _) => Ordering::Greater,
        }
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Item) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Item) -> bool {
        match (self, other) {
            (&Item::Binding { .. }, &Item::Binding { .. }) |
            (&Item::Relay { .. }, &Item::Relay { .. }) |
            (&Item::Auto { .. }, &Item::Auto { .. }) |
            (&Item::Subscriber { .. }, &Item::Subscriber { .. }) |
            (&Item::Iterator { .. }, &Item::Iterator { .. }) => true,
            _ => false,
        }
    }
}

impl Eq for Item {}

/// Actual content of a value binding
#[derive(Debug, Clone)]
pub enum Expression {
    Array(LinkedList<Expression>),
    Map(HashMap<String, Expression>),
    Reference(Path),

    /// This variant is never found in hatchet script,
    /// and is added by the compiler for each entity in the map
    Entity(String),
}

/// Defines a path to an entity or method
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Path {
    Deref(Box<Path>, String),
    Instance(Box<Path>),
    Binding(String),
}

impl Display for Path {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match self {
            &Path::Deref(ref obj, ref prop) => write!(fmt, "{}.{}", obj, prop),
            &Path::Instance(ref pat) => write!(fmt, "{}:", pat),
            &Path::Binding(ref name) => write!(fmt, "{}", name),
        }
    }
}

/// Executable script statements
#[derive(Debug, Clone)]
pub enum Statement {
    Call {
        path: Path,
        arg: Literal,
    },

    Delay {
        time: f32,
        body: LinkedList<Statement>,
    },

    Iterator {
        var: String,
        array: String,
        body: LinkedList<Statement>,
    },
}

/// Literal values, used as arguments for method calls
#[derive(Debug, Clone)]
pub enum Literal {
    Void,
    Number(f32),
    String(String),
}

impl Display for Literal {
    fn fmt(&self, fmt: &mut Formatter) -> Result<(), Error> {
        match self {
            &Literal::String(ref val) => fmt.write_str(&val),
            &Literal::Number(val) => write!(fmt, "{}", val),
            &Literal::Void => Ok(()),
        }
    }
}
