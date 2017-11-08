//! Transform a block (scoped list of statements) to LLVM IR

use atom::*;
use hct::ast::*;
use vmf::ir::Entity;

use super::builder::*;
use super::expression::*;
use super::function::*;
use super::path::*;
use super::scope::*;
use super::types::*;

fn loop_<EC, PB, LC, R>(builder: &mut Builder, entry_cond: EC, print_body: PB, loop_cond: LC)
    where EC: FnOnce(&mut Builder) -> ValueRef,
          PB: FnOnce(&mut Builder) -> R, LC: FnOnce(&mut Builder, &R) -> ValueRef {
    let entry_block = builder.get_insert_block();

    let loop_block = builder.append_basic_block();
    builder.position_at_end(loop_block);
    let r = print_body(builder);

    let end_block = builder.append_basic_block();
    let loop_cond = loop_cond(builder, &r);
    builder.build_cond_br(
        &loop_cond,
        loop_block,
        end_block,
    );

    builder.position_at_end(entry_block);
    let entry_cond = entry_cond(builder);
    builder.build_cond_br(
        &entry_cond,
        loop_block,
        end_block,
    );

    builder.position_at_end(end_block);
}

#[cfg_attr(feature="clippy", allow(too_many_arguments))]
fn iterator<EC, BP, PB, NV, LC>(
    builder: &mut Builder, scope: &Scope,
    start_val: ValueRef,
    entry_cmp: EC, build_phi: BP, print_body: PB, next_val: NV, loop_cmp: LC)
    where EC: FnOnce(&mut Builder) -> ValueRef,
          BP: FnOnce(&mut Builder) -> ValueRef,
          PB: FnOnce(&mut Builder, &Scope, &ValueRef),
          NV: FnOnce(&mut Builder, &ValueRef) -> ValueRef,
          LC: FnOnce(&mut Builder, &ValueRef) -> ValueRef {

    let prev_block = builder.get_insert_block();
    loop_(
        builder,
        entry_cmp,
        |builder| {
            let it = build_phi(builder);
            builder.add_incoming(&it, start_val, prev_block);

            print_body(builder, scope, &it);

            let body_block = builder.get_insert_block();

            let next_val = next_val(builder, &it);
            builder.add_incoming(&it, next_val.clone(), body_block);

            next_val
        },
        loop_cmp,
    );
}

/// Execute a list of script statements in order
pub fn statements<'a>(list: Vec<Statement>, mut scope: Scope<'a>, builder: &mut Builder) {
    // Hoist the entity declarations to the top of the block
    for stmt in &list {
        match *stmt {
            // Register the new Relay entities
            Statement::Relay { ref name, .. } => {
                let value = builder.build_const_entity(name);
                scope.set_binding(builder, name.clone(), &value);

                builder.add_entity(name.clone(), Entity {
                    classname: hct_atom!("logic_relay"),
                    targetname: Some(name.clone()),
                    .. Default::default()
                });
            },

            // Insert the logic_auto entity if needed
            // The empty string can be used as a name without collision risk,
            // as empty buildernames are filtered out when the AST is built
            Statement::Auto { .. } => {
                builder.add_auto_entity();
            },

            _ => {},
        }
    }

    for stmt in list {
        match stmt {
            Statement::Binding { name, value } => {
                let value = expression(value, &scope, builder);
                scope.set_binding(builder, name, &value);
            },

            Statement::Relay { name, body } => {
                let scope = scope.relay(builder, &name);
                statements(body, scope, builder);
            },

            Statement::Auto { body } => {
                statements(body, scope.auto(builder), builder);
            },

            Statement::Subscriber { path, body } => {
                let (entity, method) = event(path, &scope, builder);
                statements(body, scope.subscriber(entity, method), builder);
            },

            Statement::Delay { body, time } => {
                let time = expression(time, &scope, builder);
                let scope = scope.with_delay(builder, time);
                statements(body, scope, builder);
            },

            Statement::Loop { condition, body } => {
                let entry_cond = condition.clone();
                loop_(
                    builder,
                    |builder| {
                        let entry_cond = expression(entry_cond, &scope, builder);
                        assert_eq!(entry_cond.ty, TypeId::bool);
                        entry_cond
                    },
                    |builder| {
                        statements(
                            body,
                            scope.fork(),
                            builder,
                        );
                    },
                    |builder, _| {
                        let loop_cond = expression(condition, &scope, builder);
                        assert_eq!(loop_cond.ty, TypeId::bool);
                        loop_cond
                    },
                );
            },

            Statement::Iterator { var, array: Expression::Call(Call { path: Path::Binding(hct_atom!("range")), args }), body } => {
                let args: Vec<_> = {
                    args.into_iter()
                        .map(|arg| expression(arg, &scope, builder))
                        .collect()
                };

                assert_eq!(args.len(), 2);
                let start_val = &args[0];
                let end_val = &args[1];
                assert_eq!(start_val.ty, TypeId::f64);
                assert_eq!(end_val.ty, TypeId::f64);

                iterator(
                    builder, &scope,
                    start_val.clone(),
                    move |builder| {
                        builder.build_float_lt(
                            start_val,
                            end_val,
                        )
                    },
                    |builder| {
                        builder.build_phi(
                            TypeId::f64,
                        )
                    },
                    |builder, scope, it| {
                        statements(
                            body.clone(),
                            scope.with_binding(
                                builder,
                                var.clone(),
                                it,
                            ),
                            builder,
                        );
                    },
                    |builder, it| {
                        let inc = builder.build_const_f64(1.0);
                        builder.build_fadd(
                            it,
                            &inc,
                        )
                    },
                    move |builder, next_val| {
                        builder.build_float_lt(
                            next_val,
                            end_val,
                        )
                    },
                );
            },
            Statement::Iterator { var, array, body } => {
                let array = expression(array, &scope, builder);
                match array.ty.clone() {
                    TypeId::Array { len, ty } => {
                        let start_val = builder.build_const_i64(0);
                        let end_val = builder.build_const_i64(i64::from(len));
                        let ty = *ty;

                        iterator(
                            builder, &scope,
                            start_val.clone(),
                            |builder| {
                                builder.build_int_lt(
                                    &start_val,
                                    &end_val,
                                )
                            },
                            |builder| {
                                builder.build_phi(
                                    TypeId::i64,
                                )
                            },
                            |builder, scope, it| {
                                let value = {
                                    let zero = builder.build_const_i32(0);
                                    let gep = builder.build_in_bounds_gep(
                                        &array,
                                        vec![ &zero, it ],
                                    );

                                    let res = builder.build_load(&gep);
                                    ValueRef { ty, ptr: res.ptr }
                                };

                                statements(
                                    body,
                                    scope.with_binding(
                                        builder,
                                        var.clone(),
                                        &value,
                                    ),
                                    builder,
                                );
                            },
                            |builder, it| {
                                let inc = builder.build_const_i64(1);
                                builder.build_nswadd(
                                    it,
                                    &inc,
                                )
                            },
                            |builder, next_val| {
                                builder.build_int_lt(
                                    next_val,
                                    &end_val,
                                )
                            },
                        );
                    },

                    TypeId::Vec { ty } => {
                        let start_val = builder.build_const_i64(0);
                        let vec_len_0 = Atom::from(format!("vec_len.{:?}", ty));
                        let vec_len_1 = vec_len_0.clone();
                        let vec_get = Atom::from(format!("vec_get.{:?}", ty));

                        iterator(
                            builder, &scope,
                            start_val.clone(),
                            |builder| {
                                let vec_len = call_stl(
                                    builder,
                                    vec_len_0,
                                    vec![ &array ],
                                );

                                builder.build_int_lt(
                                    &start_val,
                                    &vec_len,
                                )
                            },
                            |builder| {
                                builder.build_phi(
                                    TypeId::i64,
                                )
                            },
                            |builder, scope, it| {
                                let value = call_stl(
                                    builder,
                                    vec_get,
                                    vec![
                                        &array,
                                        it,
                                    ],
                                );
                                statements(
                                    body.clone(),
                                    scope.with_binding(
                                        builder,
                                        var.clone(),
                                        &value,
                                    ),
                                    builder,
                                );
                            },
                            |builder, it| {
                                let inc = builder.build_const_i64(1);
                                builder.build_nswadd(
                                    it,
                                    &inc,
                                )
                            },
                            |builder, next_val| {
                                let vec_len = call_stl(
                                    builder,
                                    vec_len_1,
                                    vec![ &array ],
                                );

                                builder.build_int_lt(
                                    next_val,
                                    &vec_len,
                                )
                            },
                        );
                    },

                    _ => panic!("Tried to iterate on a non-array"),
                }

            },

            Statement::Branch { condition, consequent, alternate } => {
                let cond = expression(condition, &scope, builder);
                let entry_block = builder.get_insert_block();

                let then_block = {
                    let block = builder.append_basic_block();
                    builder.position_at_end(block);

                    statements(
                        consequent,
                        scope.fork(),
                        builder,
                    );

                    (block, builder.get_insert_block())
                };

                let alt_block = alternate.map(|alternate| {
                    let block = builder.append_basic_block();
                    builder.position_at_end(block);

                    statements(
                        alternate,
                        scope.fork(),
                        builder,
                    );

                    (block, builder.get_insert_block())
                });

                let next_block = builder.append_basic_block();

                builder.position_at_end(entry_block);
                if let Some((alt_start, alt_end)) = alt_block {
                    builder.build_cond_br(
                        &cond,
                        then_block.0,
                        alt_start,
                    );

                    builder.position_at_end(alt_end);
                    builder.build_br(next_block);
                } else {
                    builder.build_cond_br(
                        &cond,
                        then_block.0,
                        next_block,
                    );
                }

                builder.position_at_end(then_block.1);
                builder.build_br(next_block);

                builder.position_at_end(next_block);
            },

            Statement::Assignment { prop, value } => {
                let value = expression(value, &scope, builder);
                let mut prop = unwind_path(prop);

                let head = prop.pop_front().unwrap();
                let head = {
                    scope.binding_mut(&head)
                        .expect(&format!("binding \"{}\" not found", head))
                };

                match head.ty {
                    _ if prop.is_empty() => {
                        assert_eq!(head.ty, value.ty);
                        builder.build_store(&value, &head);
                    },

                    TypeId::Entity => {
                        let value = if value.ty == TypeId::String {
                            value
                        } else if value.ty == TypeId::f64 {
                            call_stl(
                                builder,
                                hct_atom!("to_string"),
                                vec![ &value ],
                            )
                        } else {
                            panic!("{:?}", value)
                        };

                        let head = builder.build_load(&head);

                        let key = prop.pop_front().unwrap();
                        let key = builder.build_const_atom(key);
                        if prop.is_empty() {
                            call_stl(
                                builder,
                                hct_atom!("set_property"),
                                vec![ &head, &key, &value ],
                            );
                        } else {
                            let sub = prop.pop_front().unwrap();
                            let sub = builder.build_const_i64(match sub {
                                hct_atom!("x") | hct_atom!("r") | hct_atom!("pitch") => 0,
                                hct_atom!("y") | hct_atom!("g") | hct_atom!("yaw") => 1,
                                hct_atom!("z") | hct_atom!("b") | hct_atom!("roll") => 2,
                                hct_atom!("w") | hct_atom!("a") => 3,
                                sub => panic!("unsupported sub-property \"{}\"", sub),
                            });

                            call_stl(
                                builder,
                                hct_atom!("set_sub_property"),
                                vec![ &head, &key, &sub, &value ],
                            );
                        }
                    },

                    _ => panic!("Unsupported assignment")
                }
            },

            Statement::Call(val) => {
                call(val, &scope, builder);
            },
        }
    }
}
