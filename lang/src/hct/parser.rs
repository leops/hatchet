use synom::*;
use synom::space::*;
use super::ast::*;
use super::expression::*;
use atom::Atom;

fn position<P>(input: &str, predicate: P) -> Option<usize> where P: Fn(&str) -> IResult<&str, &str> {
    for o in 0..input.len() {
        if let IResult::Done(_, _) = predicate(&input[o..]) {
            return Some(o);
        }
    }

    None
}

macro_rules! take_till {
    ($i:expr, $submac:ident!( $($args:tt)* )) => {
        take_till!($i, |c| $submac!(c, $($args)*))
    };
    ($i:expr, $f:expr) => {
        match position($i, $f) {
            Some(0) | None => IResult::Error,
            Some(n) => IResult::Done(&$i[n..], &$i[..n]),
        }
    };
}

named!(
    pub string -> Vec<StringPart>,
    delimited!(
        punct!("\""),
        many0!(alt!(
            delimited!(
                punct!("${"),
                expression,
                punct!("}")
            ) => { StringPart::Expression } |

            take_till!(alt!(
                tag!("\"") | tag!("${")
            )) => { |s| {
                StringPart::String(String::from(s))
            }}
        )),
        punct!("\"")
    )
);

pub fn digits(input: &str) -> IResult<&str, &str> {
    let input = skip_whitespace(input);
    let input_length = input.len();
    if input_length == 0 {
        return IResult::Error
    }

    for (idx, item) in input.chars().enumerate() {
        match item {
            '0'...'9' => {},
            _ => {
                if idx == 0 {
                    return IResult::Error
                } else {
                    return IResult::Done(&input[idx..], &input[..idx])
                }
            },
        }
    }

    IResult::Done(&input[input_length..], input)
}

macro_rules! recognize (
    ($i:expr, $submac:ident!( $($args:tt)* )) => ({
        match $submac!($i, $($args)*) {
            IResult::Done(i, _) => {
                let index = $i.len() - i.len();
                IResult::Done(i, &$i[..index])
            },
            IResult::Error => IResult::Error,
        }
    });
);

named!(
    pub number -> f64,
    map!(
        recognize!(
            tuple!(
                option!(alt!(punct!("+") | punct!("-"))),
                alt!(
                    delimited!(digits, punct!("."), option!(digits)) |
                    delimited!(option!(digits), punct!("."), digits) |
                    digits
                ),
                option!(tuple!(
                    alt!(punct!("e") | punct!("E")),
                    option!(alt!(punct!("+") | punct!("-"))),
                    digits
                ))
            )
        ),
        |v| skip_whitespace(v).parse().unwrap()
    )
);

pub fn name(input: &str) -> IResult<&str, Atom> {
    let input = skip_whitespace(input);
    let input_length = input.len();
    if input_length == 0 {
        return IResult::Error
    }

    for (idx, item) in input.chars().enumerate() {
        match item {
            'a'...'z' | 'A'...'Z' | '0'...'9' | '_' | '-' | '$' | '@' => {},
            _ => {
                if idx == 0 {
                    return IResult::Error
                } else {
                    return IResult::Done(&input[idx..], Atom::from(&input[..idx]))
                }
            },
        }
    }

    match input {
        "auto" | "relay" | "delay" |
        "while" | "for" | "in" | "if" | "else" | "let" => IResult::Error,
        name => IResult::Done(&input[input_length..], Atom::from(name)),
    }
}

named!(
    name_chain -> Vec<Atom>,
    separated_nonempty_list!(
        punct!("."),
        name
    )
);

#[inline]
fn fold_path(path: Option<Path>, name: Atom) -> Option<Path> {
    let name = Atom::from(name);
    Some(match path {
        Some(p) => Path::Deref(box p, name),
        None => Path::Binding(name),
    })
}

named!(
    path -> Path,
    do_parse!(
        inst: option!(terminated!(name_chain, punct!(":"))) >>
        pat: name_chain >>
        ({
            pat.into_iter()
                .fold(
                    inst.into_iter()
                        .flat_map(|v| v)
                        .fold(None, fold_path)
                        .map(|p| Path::Instance(box p)),
                    fold_path
                )
                .expect("empty list")
        })
    )
);

named!(
    literal -> Expression,
    alt!(
        number => { |val| Expression::Literal(Literal::Number(val)) } |
        string => { |val|  Expression::Literal(Literal::String(val)) }
    )
);

named!(
    call -> Call,
    do_parse!(
        path: path >>
        args: delimited!(
            punct!("("),
            terminated_list!(
                punct!(","),
                expression
            ),
            punct!(")")
        ) >>
        (Call { path, args })
    )
);

named!(
    array -> Expression,
    map!(
        delimited!(
            punct!("["),
            terminated_list!(
                punct!(","),
                expression
            ),
            punct!("]")
        ),
        Expression::Array
    )
);

named!(
    map -> Expression,
    map!(
        delimited!(
            punct!("{"),
            terminated_list!(
                punct!(","),
                tuple!(
                    name,
                    punct!(":"),
                    expression
                )
            ),
            punct!("}")
        ),
        |val: Vec<_>| Expression::Map(
            val.into_iter().map(|(a, _, b)| (a, b)).collect()
        )
    )
);

named!(
    pub atomic_expression -> Expression,
    alt!(
        delimited!(
            punct!("("),
            expression,
            punct!(")")
        ) |
        literal |
        call => { Expression::Call } |
        array | map |
        path => { Expression::Reference }
    )
);

fn expression(input: &str) -> IResult<&str, Expression> {
    EXPRESSION_PARSER.parse(input)
}

named!(
    block -> Vec<Statement>,
    delimited!(
        punct!("{"),
        many0!(statement),
        punct!("}")
    )
);

named!(
    auto -> Statement,
    do_parse!(
        keyword!("auto") >>
        body: block >>
        (Statement::Auto { body })
    )
);

named!(
    relay -> Statement,
    do_parse!(
        keyword!("relay") >>
        name: name >>
        body: block >>
        (Statement::Relay { name, body })
    )
);

named!(
    event -> Statement,
    do_parse!(
        path: path >>
        body: block >>
        (Statement::Subscriber { path, body })
    )
);

named!(
    delay -> Statement,
    do_parse!(
        keyword!("delay") >>
        time: expression >>
        body: block >>
        (Statement::Delay { time, body })
    )
);

named!(
    loop_ -> Statement,
    do_parse!(
        keyword!("while") >>
        condition: expression >>
        body: block >>
        (Statement::Loop { condition, body })
    )
);

named!(
    iterator -> Statement,
    do_parse!(
        keyword!("for") >>
        var: name >>
        keyword!("in") >>
        array: expression >>
        body: block >>
        (Statement::Iterator { var, array, body })
    )
);

named!(
    branch -> Statement,
    do_parse!(
        keyword!("if") >>
        condition: expression >>
        consequent: block >>
        alternate: option!(
            preceded!(
                keyword!("else"),
                alt!(
                    branch => { |alt| vec![ alt ] } |
                    block
                )
            )
        ) >>
        (Statement::Branch { condition, consequent, alternate })
    )
);

named!(
    binding -> Statement,
    do_parse!(
        keyword!("let") >>
        name: name >>
        punct!("=") >>
        value: expression >>
        (Statement::Binding { name, value })
    )
);

named!(
    assignment -> Statement,
    do_parse!(
        prop: path >>
        punct!("=") >>
        value: expression >>
        (Statement::Assignment { prop, value })
    )
);

named!(
    call_statement -> Statement,
    map!(call, Statement::Call)
);

named!(
    statement -> Statement,
    alt!(
        auto | relay | event | delay |
        loop_ | iterator | branch | binding | assignment |
        call_statement
    )
);

named!(
    pub script -> Script,
    map!(many0!(statement), |body| Script { body })
);
