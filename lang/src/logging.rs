use std::io::{self, Write as IOWrite};
use std::fmt::{self, Display, Write as FMTWrite};
use std::time::Duration;
use std::path::Path as StdPath;
use std::cmp;

use rayon::prelude::*;
use simplelog::LogLevel::Trace;
use either::*;
use diff;

use term::*;
use term::Result as TermResult;
use term::color::*;

use hct::ast::*;

macro_rules! timer_start {
    () => (
        if log_enabled!(::log::LogLevel::Debug) {
            Some(::std::time::Instant::now())
        } else {
            None
        }
    );
}

macro_rules! timer_chain {
    ($prev:ident, $time:ident, $( $rest:tt )* ) => (
        $prev.map(|prev| {
            let $time = $crate::logging::duration_fmt(prev.elapsed());
            debug!( $( $rest )* );
            ::std::time::Instant::now()
        })
    );
}

macro_rules! timer_end {
    ($timer:ident, $time:ident, $( $rest:tt )* ) => (
        if let Some(timer) = $timer {
            let $time = $crate::logging::duration_fmt(timer.elapsed());
            debug!( $( $rest )* );
        }
    );
}

pub fn path_fmt<P: AsRef<StdPath>>(path: P) -> String {
    if log_enabled!(Trace) {
        path.as_ref().display().to_string()
    } else {
        path.as_ref().file_name().unwrap().to_string_lossy().to_string()
    }
}

pub fn duration_fmt(duration: Duration) -> String {
    let mut res = String::new();

    let secs = duration.as_secs();
    if secs > 0 {
        write!(res, "{}s ", secs).unwrap();
    }

    let nano = duration.subsec_nanos();
    let milli = nano / 1_000_000;
    if milli > 0 {
        write!(res, "{}ms ", milli).unwrap();
    }

    let micro = nano.checked_rem(milli * 1_000_000).unwrap_or(nano) / 1_000;
    if micro > 0 {
        write!(res, "{}μs ", micro).unwrap();
    }

    if log_enabled!(Trace) {
        let nanos = nano.checked_rem(((milli * 1_000) + micro) * 1_000).unwrap_or(nano);
        if nanos > 0 {
            write!(res, "{}ns", nanos).unwrap();
        }
    }

    res
}

pub fn print_diff(then: &str, now: &str) {
    let lines = diff::lines(then, now);

    let (l_max, r_max) = {
        lines.par_iter()
            .map(|line| match *line {
                diff::Result::Left(l) => (l.len(), 0),
                diff::Result::Both(l, r) => (l.len(), r.len()),
                diff::Result::Right(r) => (0, r.len()),
            })
            .reduce(
                || (0, 0),
                |(a_l, a_r), (b_l, b_r)| (
                    cmp::max(a_l, b_l),
                    cmp::max(a_r, b_r),
                ),
            )
    };

    let mut t = stdout().unwrap_or_else(|| box BasicTerm::new());
    if l_max + r_max > 237 {
        for line in lines {
            match line {
                diff::Result::Left(val) => {
                    t.fg(color::RED).unwrap();
                    writeln!(t, "- {}", val).unwrap();
                },
                diff::Result::Right(val) => {
                    t.fg(color::GREEN).unwrap();
                    writeln!(t, "+ {}", val).unwrap();
                },

                diff::Result::Both(val, _) => {
                    t.reset().unwrap();
                    writeln!(t, "  {}", val).unwrap();
                },
            }
        }

        t.reset().unwrap();
    } else {
        let (l, r) = {
            lines.into_iter()
                .fold((Vec::new(), Vec::new()), |(mut l, mut r), line| {
                    match line {
                        diff::Result::Left(val) => l.push(Right(val)),
                        diff::Result::Right(val) => r.push(Right(val)),

                        diff::Result::Both(val, _) => {
                            while l.len() < r.len() {
                                l.push(Left(""));
                            }
                            while r.len() < l.len() {
                                r.push(Left(""));
                            }

                            l.push(Left(val));
                            r.push(Left(val));
                        },
                    }

                    (l, r)
                })
        };

        for (l, r) in l.into_iter().zip(r.into_iter()) {
            match l {
                Left(val) => {
                    write!(t, "{:w$} ", val, w = l_max).unwrap();
                },
                Right(val) => {
                    t.fg(color::RED).unwrap();
                    write!(t, "{:w$} ", val, w = l_max).unwrap();
                    t.reset().unwrap();
                },
            }

            match r {
                Left(val) => {
                    writeln!(t, "{}", val).unwrap();
                },
                Right(val) => {
                    t.fg(color::GREEN).unwrap();
                    writeln!(t, "{}", val).unwrap();
                    t.reset().unwrap();
                },
            }
        }
    }
}

enum ColorType {
    Number,
    String,
    Function,
    Name,
    Keyword,
}

fn print_colored<T: Display>(fmt: &mut Terminal<Output=io::Stdout>, val: T, color: ColorType) -> TermResult<()> {
    fmt.fg(match color {
        ColorType::Number => YELLOW,
        ColorType::String => GREEN,
        ColorType::Function => CYAN,
        ColorType::Name => RED,
        ColorType::Keyword => MAGENTA,
    })?;

    write!(fmt, "{}", val)?;

    fmt.fg(BRIGHT_BLACK)
}

macro_rules! write_col {
    ( $fmt:expr, Path( $val:expr, $is_func:expr ) $( $rest:tt )* ) => {{
        print_path($fmt, $val, $is_func)?;
        write_col!( $fmt $( $rest )* )
    }};
    ( $fmt:expr, Expression( $val:expr ) $( $rest:tt )* ) => {{
        print_expression($fmt, $val)?;
        write_col!( $fmt $( $rest )* )
    }};
    ( $fmt:expr, Statement( $val:expr ) $( $rest:tt )* ) => {{
        print_statement($fmt, $val)?;
        write_col!( $fmt $( $rest )* )
    }};
    ( $fmt:expr, $col:ident( $val:expr ) $( $rest:tt )* ) => {{
        print_colored($fmt, $val, ColorType::$col)?;
        write_col!( $fmt $( $rest )* )
    }};
    ( $fmt:expr, $val:expr, $( $rest:tt )* ) => {{
        write!($fmt, "{}", $val)?;
        write_col!( $fmt, $( $rest )* )
    }};

    ( $fmt:expr, $val:expr ) => {
        Ok(write!($fmt, "{}", $val)?) as TermResult<()>
    };
    ( $fmt:expr, ) => { Ok(()) as TermResult<()> };
    ( $fmt:expr ) => { Ok(()) as TermResult<()> };
}
macro_rules! writeln_col {
    ( $( $args:tt )* ) => {
        write_col!( $( $args )* , "\n")
    };
}

fn print_literal(fmt: &mut Terminal<Output=io::Stdout>, lit: &Literal) -> TermResult<()> {
    match *lit {
        Literal::Number(ref val) => write_col!(fmt, Number(val)),
        Literal::String(ref val) => {
            write_col!(fmt, String("\""))?;
            for part in val {
                match *part {
                    StringPart::Expression(ref e) => {
                        write_col!(fmt, String("${"))?;
                        print_expression(fmt, e)?;
                        write_col!(fmt, String("}"))?;
                    },
                    StringPart::String(ref s) => write_col!(fmt, String(s))?,
                }
            }
            write_col!(fmt, String("\""))
        },
    }
}

fn print_path(fmt: &mut Terminal<Output=io::Stdout>, path: &Path, is_func: bool) -> TermResult<()> {
    match *path {
        Path::Deref(ref obj, ref prop) => {
            print_path(fmt, obj, false)?;
            if let Path::Instance(_) = **obj {} else {
                write!(fmt, ".")?;
            }
            print_colored(fmt, prop, if is_func { ColorType::Function } else { ColorType::Name })
        },
        Path::Instance(ref pat) => write_col!(fmt, Path(pat, false), ":"),
        Path::Binding(ref name) => {
            print_colored(fmt, name, if is_func { ColorType::Function } else { ColorType::Name })
        },
    }
}

fn print_call(fmt: &mut Terminal<Output=io::Stdout>, call: &Call) -> TermResult<()> {
    write_col!(fmt, Path(&call.path, true), "(")?;
    for (i, arg) in call.args.iter().enumerate() {
        print_expression(fmt, arg)?;
        if i < call.args.len() - 1 {
            write!(fmt, ", ")?;
        }
    }

    Ok(write!(fmt, ")")?)
}

fn print_expression(fmt: &mut Terminal<Output=io::Stdout>, exp: &Expression) -> TermResult<()> {
    use hct::ast::Expression::*;
    match *exp {
        Call(ref call) => print_call(fmt, call),
        Reference(ref path) => print_path(fmt, path, false),
        Binary { ref lhs, ref op, ref rhs } => {
            write_col!(fmt, "(", Expression(lhs), " ", op, " ", Expression(rhs), ")")
        },
        Literal(ref lit) => print_literal(fmt, lit),

        Array(ref items) => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln!(fmt, "[")?;
                for item in items {
                    writeln_col!(&mut fmt, Expression(item), ",")?;
                }
            }
            Ok(write!(fmt, "]")?)
        },
        Map(ref items) => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln!(fmt, "{{")?;
                for (k, v) in items {
                    writeln_col!(&mut fmt, k, ": ", Expression(v), ",")?;
                }
            }
            Ok(write!(fmt, "}}")?)
        }
    }
}

fn print_statement(fmt: &mut Terminal<Output=io::Stdout>, stmt: &Statement) -> TermResult<()> {
    use hct::ast::Statement::*;
    match *stmt {
        Auto { ref body } => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln_col!(&mut fmt, Keyword("auto"), " {")?;
                for stmt in body {
                    writeln_col!(&mut fmt, Statement(stmt))?;
                }
            }
            Ok(write!(fmt, "}}")?)
        },

        Relay { ref name, ref body } => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln_col!(&mut fmt, Keyword("relay"), " ", name, " {")?;
                for stmt in body {
                    writeln_col!(&mut fmt, Statement(stmt))?;
                }
            }
            Ok(write!(fmt, "}}")?)
        },
        Subscriber { ref path, ref body } => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln_col!(&mut fmt, Path(path, true), " {")?;
                for stmt in body {
                    writeln_col!(&mut fmt, Statement(stmt))?;
                }
            }
            Ok(write!(fmt, "}}")?)
        },
        Delay { ref time, ref body } => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln_col!(&mut fmt, Keyword("delay"), " ", Expression(time), " {")?;
                for stmt in body {
                    writeln_col!(&mut fmt, Statement(stmt))?;
                }
            }
            Ok(write!(fmt, "}}")?)
        },

        Loop { ref condition, ref body } => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln_col!(&mut fmt, Keyword("while"), " ", Expression(condition), " {")?;
                for stmt in body {
                    writeln_col!(&mut fmt, Statement(stmt))?;
                }
            }
            Ok(write!(fmt, "}}")?)
        },
        Iterator { ref var, ref array, ref body } => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln_col!(&mut fmt, Keyword("for"), " ", Name(var), " ", Keyword("in"), " ", Expression(array), " {")?;
                for stmt in body {
                    writeln_col!(&mut fmt, Statement(stmt))?;
                }
            }
            Ok(write!(fmt, "}}")?)
        },
        Branch { ref condition, ref consequent, ref alternate } => {
            {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln_col!(&mut fmt, Keyword("if"), " ", Expression(condition), " {")?;
                for stmt in consequent {
                    writeln_col!(&mut fmt, Statement(stmt))?;
                }
            }

            if let Some(ref alternate) = *alternate {
                let mut fmt = PadAdapterTerm::new(fmt);
                writeln_col!(&mut fmt, "} ", Keyword("else"), " {")?;
                for stmt in alternate {
                    writeln_col!(&mut fmt, Statement(stmt))?;
                }
            }

            Ok(write!(fmt, "}}")?)
        },

        Binding { ref name, ref value } => {
            write_col!(fmt, Keyword("let"), " ", Name(name), " = ", Expression(value))
        },
        Assignment { ref prop, ref value } => {
            write_col!(fmt, Path(prop, false), " = ", Expression(value))
        },

        Call(ref call) => print_call(fmt, call),
    }
}

pub fn print_script(script: &Script) -> TermResult<()> {
    use std::ops::DerefMut;

    let mut fmt = stdout().unwrap_or_else(|| box BasicTerm::new());
    for stmt in &script.body {
        writeln_col!(fmt.deref_mut(), Statement(stmt))?;
    }

    fmt.reset()?;
    Ok(())
}

/// Utility struct for pretty-printing blocks
/// from https://github.com/rust-lang/rust/blob/master/src/libcore/fmt/builders.rs
pub struct PadAdapter<'a> {
    fmt: &'a mut FMTWrite,
    on_newline: bool,
}

impl<'a> PadAdapter<'a> {
    pub fn new(fmt: &'a mut FMTWrite) -> PadAdapter<'a> {
        PadAdapter {
            fmt,
            on_newline: false,
        }
    }
}

impl<'a> FMTWrite for PadAdapter<'a> {
    fn write_str(&mut self, mut s: &str) -> fmt::Result {
        while !s.is_empty() {
            if self.on_newline {
                self.fmt.write_str("    ")?;
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

pub struct PadAdapterTerm<'a> {
    fmt: &'a mut Terminal<Output=io::Stdout>,
    on_newline: bool,
}

impl<'a> PadAdapterTerm<'a> {
    pub fn new(fmt: &'a mut Terminal<Output=io::Stdout>) -> PadAdapterTerm<'a> {
        PadAdapterTerm {
            fmt,
            on_newline: false,
        }
    }
}

impl<'a> IOWrite for PadAdapterTerm<'a> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let res = buf.len();
        while !buf.is_empty() {
            if self.on_newline {
                self.fmt.write_all(b"    ")?;
            }

            let split = match buf.iter().enumerate().find(|&(_, c)| *c == b'\n') {
                Some((pos, _)) => {
                    self.on_newline = true;
                    pos + 1
                }
                None => {
                    self.on_newline = false;
                    buf.len()
                }
            };

            self.fmt.write_all(&buf[..split])?;
            buf = &buf[split..];
        }

        Ok(res)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.fmt.flush()
    }
}

impl<'a> Terminal for PadAdapterTerm<'a> {
    type Output = io::Stdout;
    fn fg(&mut self, color: Color) -> Result<()> {
        self.fmt.fg(color)
    }
    fn bg(&mut self, color: Color) -> Result<()> {
        self.fmt.bg(color)
    }
    fn attr(&mut self, attr: Attr) -> Result<()> {
        self.fmt.attr(attr)
    }
    fn supports_attr(&self, attr: Attr) -> bool {
        self.fmt.supports_attr(attr)
    }
    fn reset(&mut self) -> Result<()> {
        self.fmt.reset()
    }
    fn supports_reset(&self) -> bool {
        self.fmt.supports_reset()
    }
    fn supports_color(&self) -> bool {
        self.fmt.supports_color()
    }
    fn cursor_up(&mut self) -> Result<()> {
        self.fmt.cursor_up()
    }
    fn delete_line(&mut self) -> Result<()> {
        self.fmt.delete_line()
    }
    fn carriage_return(&mut self) -> Result<()> {
        self.fmt.carriage_return()
    }
    fn get_ref(&self) -> &Self::Output {
        self.fmt.get_ref()
    }
    fn get_mut(&mut self) -> &mut Self::Output {
        self.fmt.get_mut()
    }
    fn into_inner(self) -> Self::Output where Self: Sized {
        unimplemented!()
    }
}

struct BasicTerm {
    fmt: io::Stdout,
}

impl BasicTerm {
    fn new() -> BasicTerm {
        BasicTerm {
            fmt: io::stdout()
        }
    }
}

impl IOWrite for BasicTerm {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.fmt.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.fmt.flush()
    }
}

impl Terminal for BasicTerm {
    type Output = io::Stdout;
    fn fg(&mut self, _color: Color) -> Result<()> {
        Ok(())
    }
    fn bg(&mut self, _color: Color) -> Result<()> {
        Ok(())
    }
    fn attr(&mut self, _attr: Attr) -> Result<()> {
        Ok(())
    }
    fn supports_attr(&self, _attr: Attr) -> bool {
        false
    }
    fn reset(&mut self) -> Result<()> {
        Ok(())
    }
    fn supports_reset(&self) -> bool {
        false
    }
    fn supports_color(&self) -> bool {
        false
    }
    fn cursor_up(&mut self) -> Result<()> {
        Ok(())
    }
    fn delete_line(&mut self) -> Result<()> {
        Ok(())
    }
    fn carriage_return(&mut self) -> Result<()> {
        Ok(())
    }
    fn get_ref(&self) -> &Self::Output {
        &self.fmt
    }
    fn get_mut(&mut self) -> &mut Self::Output {
        &mut self.fmt
    }
    fn into_inner(self) -> Self::Output where Self: Sized {
        self.fmt
    }
}
