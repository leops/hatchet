//! Function call utilities

use atom::*;
use hct::ast::*;
use super::builder::*;
use super::expression::*;
use super::path::*;
use super::scope::*;
use super::types::*;

/// Create a call to an STL function
pub fn call_stl<'a, A>(builder: &mut Builder, name: Atom, args: A) -> ValueRef
    where A: IntoIterator<Item=&'a ValueRef>, Type: 'a, Value: 'a {
    let mut args = args.into_iter().peekable();

    match name {
        hct_atom!("length") => {
            let sum = {
                args.fold(None, |prev: Option<ValueRef>, val| {
                    let two = builder.build_const_f64(2.0);
                    let pow = call_stl(
                        builder,
                        hct_atom!("pow"),
                        vec![ val, &two ],
                    );

                    if let Some(prev) = prev {
                        Some(builder.build_fadd(&prev, &pow))
                    } else {
                        Some(pow)
                    }
                })
            };

            return call_stl(builder, hct_atom!("sqrt"), sum.iter());
        },

        hct_atom!("to_string") => {
            if let Some(val) = args.peek() {
                if let Some(val) = builder.get_const_f64(val) {
                    return builder.build_const_string(val.to_string());
                }
            }
        },

        _ => {},
    }

    //trace!("{}", name);

    let ext = builder.get_external(name);
    let ctx = match ext.args.first() {
        Some(arg) if *arg == TypeId::Context => {
            Some(builder.get_runtime_context())
        },
        _ => None,
    };

    let args = {
        ctx.into_iter()
            .chain(args.cloned())
            .zip(
                ext.args.iter().cloned()
            )
            //.inspect(|arg| trace!("{:?}", arg))
            .enumerate()
            .map(|(i, (val, arg))| {
                if val.ty == arg {
                    Ok(val)
                } else {
                    Err(format!("Expected type {:?} for argument {}, found {:?}", val.ty, i, arg))
                }
            })
            .collect::<Result<_, _>>()
            .unwrap()
    };

    builder.build_call(ext, args)
}

/// Execute an AST Call node
pub fn call<'a>(Call { path, args }: Call, scope: &Scope<'a>, builder: &mut Builder) -> ValueRef {
    let args = {
        args.into_iter()
            .map(|arg| expression(arg, scope, builder))
            .collect::<Vec<_>>()
    };

    if let Some((from, trigger)) = scope.event() {
        if from.ty == TypeId::Entity {
            let (entity, method) = event(path, scope, builder);
            let delay = scope.delay().unwrap_or_else(|| builder.build_const_f64(0.0));
            let arg = {
                args.get(0)
                    .map(|val| match val.ty {
                        TypeId::String => val.clone(),
                        TypeId::f64 => call_stl(
                            builder,
                            hct_atom!("to_string"),
                            vec![ val ],
                        ),

                        ref ty => panic!("Unsupported event argument {:?}", ty),
                    })
                    .unwrap_or_else(|| builder.build_const_string(String::from("")))
            };

            return call_stl(
                builder,
                hct_atom!("create_connection"),
                vec![
                    &from,
                    &trigger,
                    &entity,
                    &method,
                    &arg,
                    &delay,
                    // TODO: once
                ],
            );
        }
    }

    let name = match path {
        Path::Binding(name) => name,
        ref path => {
            panic!("not yet implemented: {:?}", path)
        },
    };

    call_stl(builder, name, args.iter())
}
