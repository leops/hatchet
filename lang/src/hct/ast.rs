//! Defines the Hatchet language abstract syntax tree

use std::fmt::{self, Display, Formatter};
use std::collections::HashMap;
use atom::Atom;

/// Main script entry point
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Script {
    pub body: Vec<Statement>,
}

/// Function call statement / expression
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Call {
    pub path: Path,
    pub args: Vec<Expression>,
}

/// Binary operator
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Operator {
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Shl,
    Shr,
    Lt,
    Lte,
    Gte,
    Gt,
    Eq,
    Neq,
    BAnd,
    BXor,
    BOr,
    And,
    Or,
}

impl Display for Operator {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match *self {
            Operator::Mul => write!(fmt, "*"),
            Operator::Div => write!(fmt, "/"),
            Operator::Mod => write!(fmt, "%"),
            Operator::Add => write!(fmt, "+"),
            Operator::Sub => write!(fmt, "-"),
            Operator::Shl => write!(fmt, "<<"),
            Operator::Shr => write!(fmt, ">>"),
            Operator::Lt => write!(fmt, "<"),
            Operator::Lte => write!(fmt, "<="),
            Operator::Gte => write!(fmt, ">="),
            Operator::Gt => write!(fmt, ">"),
            Operator::Eq => write!(fmt, "=="),
            Operator::Neq => write!(fmt, "!="),
            Operator::BAnd => write!(fmt, "&"),
            Operator::BXor => write!(fmt, "^"),
            Operator::BOr => write!(fmt, "|"),
            Operator::And => write!(fmt, "&&"),
            Operator::Or => write!(fmt, "||"),
        }
    }
}

/// Script expression resolving to a value
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expression {
    Call(Call),

    Array(Vec<Expression>),
    Map(HashMap<Atom, Expression>),

    Reference(Path),

    Binary {
        lhs: Box<Expression>,
        op: Operator,
        rhs: Box<Expression>,
    },

    Literal(Literal),
}

/// Defines a path to an entity or method
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Path {
    Deref(Box<Path>, Atom),
    Instance(Box<Path>),
    Binding(Atom),
}

impl Display for Path {
    fn fmt(&self, fmt: &mut Formatter) -> fmt::Result {
        match *self {
            Path::Deref(ref obj, ref prop) => {
                if let Path::Instance(_) = **obj {
                    write!(fmt, "{}{}", obj, prop)
                } else {
                    write!(fmt, "{}.{}", obj, prop)
                }
            },
            Path::Instance(ref pat) => write!(fmt, "{}:", pat),
            Path::Binding(ref name) => write!(fmt, "{}", name),
        }
    }
}

/// Executable script statements
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Auto {
        body: Vec<Statement>,
    },
    Relay {
        name: Atom,
        body: Vec<Statement>,
    },
    Subscriber {
        path: Path,
        body: Vec<Statement>,
    },

    Delay {
        time: Expression,
        body: Vec<Statement>,
    },

    Loop {
        condition: Expression,
        body: Vec<Statement>,
    },
    Iterator {
        var: Atom,
        array: Expression,
        body: Vec<Statement>,
    },
    Branch {
        condition: Expression,
        consequent: Vec<Statement>,
        alternate: Option<Vec<Statement>>,
    },
    Binding {
        name: Atom,
        value: Expression,
    },
    Assignment {
        prop: Path,
        value: Expression,
    },
    Call(Call),
}

/// Literal values, used as arguments for method calls
#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Number(f64),
    String(Vec<StringPart>),
}

impl Eq for Literal {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StringPart {
    Expression(Expression),
    String(String),
}
