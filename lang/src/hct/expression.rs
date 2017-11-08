use synom::IResult;
use either::*;

use super::ast::{Operator, Expression, Literal};

type Parser = Box<Fn(&str) -> IResult<&str, Expression> + Sync + 'static>;
type PunctParser = Box<Fn(&str) -> IResult<&str, &str> + Sync + 'static>;
type Stack<'a> = Vec<(&'a str, Either<Expression, Operator>)>;
type Op = (Operator, usize, PunctParser);

pub struct OpParser {
    op_parsers: Vec<(Operator, PunctParser)>,
    sorted_ops: Vec<(usize, Vec<Operator>)>,
    atom_parser: Parser,
}

impl OpParser {
    pub fn new(atom_parser: Parser, operators: Vec<Op>) -> Self {
        let sorted_ops: Vec<(usize, Vec<Operator>)> = {
            operators.iter()
                .fold(
                    Vec::new(),
                    |mut list, &(op, precedence, _)| {
                        match list.binary_search_by_key(&precedence, |&(p, _)| p) {
                            Ok(i) => {
                                let &mut (_, ref mut vec) = &mut list[i];
                                vec.push(op);
                            },
                            Err(i) => list.insert(i, (precedence, vec![op])),
                        }

                        list
                    }
                )
        };

        let op_parsers: Vec<_> = {
            operators.into_iter()
                .map(|(op, _, parser)| (op, parser))
                .collect()
        };

        OpParser {
            op_parsers,
            sorted_ops,
            atom_parser,
        }
    }

    fn shift<'a>(&self, stack: &mut Stack<'a>, i: &'a str) -> Option<&'a str> {
        for &(op, ref parser) in &self.op_parsers {
            if let IResult::Done(s, _) = (*parser)(i) {
                stack.push((s, Either::Right(op)));
                return Some(s);
            }
        }

        if let IResult::Done(s, m) = (*self.atom_parser)(i) {
            stack.push((s, Either::Left(m)));
            return Some(s);
        }

        None
    }

    pub fn parse<'a>(&self, mut input: &'a str) -> IResult<&'a str, Expression> {
        let mut stack = Vec::new();
        while let Some(rest) = self.shift(&mut stack, input) {
            input = rest;
        }

        for &(_, ref ops) in &self.sorted_ops {
            let mut i = 0;
            while i < stack.len() {
                match stack[i] {
                    (_, Either::Right(op)) if ops.contains(&op) => {
                        let (from, to, val) = match (i.checked_sub(1).and_then(|i| stack.get(i)), op, stack.get(i + 1)) {
                            (Some(&(_, ref left)), op, Some(&(rem, ref right))) => {
                                (i - 1, i + 2, Some((rem, Either::Left(
                                    Expression::Binary {
                                        lhs: box left.clone().left().unwrap(),
                                        op,
                                        rhs: box right.clone().left().unwrap(),
                                    }
                                ))))
                            },
                            (None, Operator::Sub, Some(&(rem, Either::Left(Expression::Literal(Literal::Number(val)))))) => {
                                (i, i + 2, Some((rem, Either::Left(
                                    Expression::Literal(Literal::Number(-val))
                                ))))
                            },
                            (lhs, op, rhs) => panic!("{:?} {:?} {:?}", lhs, op, rhs),
                        };

                        stack.splice(from..to, val);
                    },

                    _ => {
                        i += 1;
                    },
                }
            }
        }

        if stack.is_empty() {
            IResult::Error
        } else {
            let (rem, expr) = stack.remove(0);
            IResult::Done(rem, expr.left().unwrap())
        }
    }
}

lazy_static!{
    pub static ref EXPRESSION_PARSER: OpParser = {
        use super::ast::Operator::*;
        use super::parser::atomic_expression;

        OpParser::new(
            box atomic_expression,
            vec![
                (Mul,  5,  box |i| punct!(i, "*")),
                (Div,  5,  box |i| punct!(i, "/")),
                (Mod,  5,  box |i| punct!(i, "%")),
                (Add,  6,  box |i| punct!(i, "+")),
                (Sub,  6,  box |i| punct!(i, "-")),
                (Shl,  7,  box |i| punct!(i, "<<")),
                (Shr,  7,  box |i| punct!(i, ">>")),
                (Lte,  8,  box |i| punct!(i, "<=")),
                (Lt,   8,  box |i| punct!(i, "<")),
                (Gte,  8,  box |i| punct!(i, ">=")),
                (Gt,   8,  box |i| punct!(i, ">")),
                (Eq,   9,  box |i| punct!(i, "==")),
                (Neq,  9,  box |i| punct!(i, "!=")),
                (And,  13, box |i| punct!(i, "&&")),
                (Or,   14, box |i| punct!(i, "||")),
                (BAnd, 10, box |i| punct!(i, "&")),
                (BXor, 11, box |i| punct!(i, "^")),
                (BOr,  12, box |i| punct!(i, "|")),
            ],
        )
    };
}
