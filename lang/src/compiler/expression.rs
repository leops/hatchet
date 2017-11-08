//! Resolve the value of an expression

use atom::*;
use hct::ast::*;

use super::builder::*;
use super::function::*;
use super::scope::*;
use super::types::*;

/// Compute the return value of an expression
pub fn expression<'a>(exp: Expression, scope: &Scope<'a>, builder: &mut Builder) -> ValueRef {
    match exp {
        Expression::Call(val) => call(val, scope, builder),

        Expression::Reference(Path::Binding(name)) => {
            scope.binding(builder, &name)
                .expect(&format!("value \"{}\" not found", name))
        },
        Expression::Reference(Path::Deref(box obj, prop)) => {
            let obj = expression(Expression::Reference(obj), scope, builder);
            match &obj.ty {
                &TypeId::Object { ref items } => {
                    let idx = {
                        items.binary_search_by(|&(ref item, _)| item.cmp(&prop))
                            .expect(&format!("key \"{}\" not found in object", prop))
                    };

                    let (_, ref ty) = items[idx];
                    let zero = builder.build_const_i32(0i32);
                    let idx = builder.build_const_i32(idx as i32);
                    let gep = builder.build_in_bounds_gep(
                        &obj,
                        vec![
                            &zero,
                            &idx,
                        ],
                    );

                    let res = builder.build_load(&gep);
                    ValueRef { ty: ty.clone(), ptr: res.ptr }
                },

                &TypeId::Entity => {
                    let prop = builder.build_const_atom(prop);
                    call_stl(
                        builder,
                        hct_atom!("get_property"),
                        vec![ &obj, &prop ],
                    )
                },

                &TypeId::String => {
                    let sub = builder.build_const_i64(match prop {
                        hct_atom!("x") | hct_atom!("r") | hct_atom!("pitch") => 0,
                        hct_atom!("y") | hct_atom!("g") | hct_atom!("yaw") => 1,
                        hct_atom!("z") | hct_atom!("b") | hct_atom!("roll") => 2,
                        hct_atom!("w") | hct_atom!("a") => 3,
                        ref sub => panic!("unsupported sub-property \"{}\"", sub),
                    });

                    call_stl(
                        builder,
                        hct_atom!("get_sub_property"),
                        vec![ &obj, &sub ],
                    )
                },

                ty => panic!("cannot dereference a {:?}", ty),
            }
        },
        Expression::Reference(val) => unimplemented!("{:?}", val),

        Expression::Array(val) => {
            let values: Vec<_> = {
                val.into_iter()
                    .map(|elem| expression(elem, scope, builder))
                    .collect()
            };

            let ty = TypeId::Array {
                len: values.len() as u32,
                ty: box {
                    values.iter()
                        .fold(None, |prev, val| {
                            if let Some(prev) = prev {
                                assert_eq!(prev, val.ty, "heterogeneous array literal");
                                Some(prev)
                            } else {
                                Some(val.ty.clone())
                            }
                        })
                        .unwrap_or(TypeId::Void)
                },
            };

            let arr_elem = builder.get_element_type(&ty);
            let init = builder.build_undef(&arr_elem);

            let ptr = {
                values.into_iter()
                    .enumerate()
                    .fold(init, |prev, (i, val)| {
                        builder.build_insert_value(
                            &prev, &val, i as _,
                        )
                    })
            };

            let mut alloc = builder.build_alloca(&arr_elem);
            alloc.ty = ty;

            builder.build_store(&ptr, &alloc);

            alloc
        },
        Expression::Map(val) => {
            let mut data: Vec<_> = {
                val.into_iter()
                    .map(|(k, v)| {
                        let val = expression(v, scope, builder);
                        ((k.clone(), val.ty.clone()), val)
                    })
                    .collect()
            };

            data.sort_by(|&((ref a, _), _), &((ref b, _), _)| a.cmp(b));
            let (items, values): (Vec<_>, Vec<_>) = {
                data.into_iter().unzip()
            };

            let ty = TypeId::Object { items };

            let obj_elem = builder.get_element_type(&ty);
            let init = builder.build_undef(&obj_elem);

            let ptr = {
                values.into_iter()
                    .enumerate()
                    .fold(init, |prev, (i, val)| {
                        builder.build_insert_value(
                            &prev, &val, i as _,
                        )
                    })
            };

            let mut alloc = builder.build_alloca(&obj_elem);
            alloc.ty = ty;

            builder.build_store(&ptr, &alloc);

            alloc
        },

        Expression::Binary { lhs: box Expression::Binary { lhs: box ref e1, op: Operator::Mul, rhs: box ref e2 }, op: Operator::Add, rhs: box ref e3 } |
        Expression::Binary { lhs: box ref e3, op: Operator::Add, rhs: box Expression::Binary { lhs: box ref e1, op: Operator::Mul, rhs: box ref e2 } } => {
            let e1 = expression(e1.clone(), scope, builder);
            let e2 = expression(e2.clone(), scope, builder);
            let e3 = expression(e3.clone(), scope, builder);
            call_stl(
                builder,
                hct_atom!("fmuladd"),
                vec![ &e1, &e2, &e3 ],
            )
        },
        Expression::Binary { box lhs, op, box rhs } => {
            let lhs = expression(lhs, scope, builder);
            let rhs = expression(rhs, scope, builder);
            match (&lhs.ty, op, &rhs.ty) {
                (&TypeId::String, Operator::Add, &TypeId::String) => call_stl(
                    builder,
                    hct_atom!("concat"),
                    vec![
                        &lhs,
                        &rhs,
                    ],
                ),

                (&TypeId::f64, Operator::Add, &TypeId::f64) => builder.build_fadd(&lhs, &rhs),
                (&TypeId::f64, Operator::Sub, &TypeId::f64) => builder.build_fsub(&lhs, &rhs),
                (&TypeId::f64, Operator::Mul, &TypeId::f64) => builder.build_fmul(&lhs, &rhs),
                (&TypeId::f64, Operator::Div, &TypeId::f64) => builder.build_fdiv(&lhs, &rhs),
                (&TypeId::f64, Operator::Mod, &TypeId::f64) => builder.build_frem(&lhs, &rhs),

                (&TypeId::i64, Operator::Shl, &TypeId::i64) => builder.build_shl(&lhs, &rhs),
                (&TypeId::i64, Operator::Shr, &TypeId::i64) => builder.build_lshr(&lhs, &rhs),

                (&TypeId::f64, Operator::Lt, &TypeId::f64) => builder.build_float_lt(&lhs, &rhs),
                (&TypeId::f64, Operator::Lte, &TypeId::f64) => builder.build_float_le(&lhs, &rhs),
                (&TypeId::f64, Operator::Gt, &TypeId::f64) => builder.build_float_gt(&lhs, &rhs),
                (&TypeId::f64, Operator::Gte, &TypeId::f64) => builder.build_float_ge(&lhs, &rhs),

                (&TypeId::bool, Operator::And, &TypeId::bool) |
                (&TypeId::i64, Operator::BAnd, &TypeId::i64) => builder.build_and(&lhs, &rhs),
                (&TypeId::bool, Operator::Or, &TypeId::bool) |
                (&TypeId::i64, Operator::BOr, &TypeId::i64) => builder.build_or(&lhs, &rhs),
                (&TypeId::i64, Operator::BXor, &TypeId::i64) => builder.build_xor(&lhs, &rhs),

                (&TypeId::f64, Operator::Eq, &TypeId::f64) => builder.build_float_eq(&lhs, &rhs),
                (&TypeId::f64, Operator::Neq, &TypeId::f64) => builder.build_float_ne(&lhs, &rhs),

                (&TypeId::bool, Operator::Eq, &TypeId::bool) |
                (&TypeId::i64, Operator::Eq, &TypeId::i64) => builder.build_int_eq(&lhs, &rhs),
                (&TypeId::bool, Operator::Neq, &TypeId::bool) |
                (&TypeId::i64, Operator::Neq, &TypeId::i64) => builder.build_int_ne(&lhs, &rhs),

                (lty, op @ Operator::Neq, rty) |
                (lty, op @ Operator::Eq, rty) if lty == rty => {
                    let res = call_stl(
                        builder,
                        Atom::from(format!("eq.{:?}", lty)),
                        vec![
                            &lhs,
                            &rhs,
                        ],
                    );

                    if op == Operator::Neq {
                        builder.build_not(&res)
                    } else {
                        res
                    }
                },

                (lty, op, rty) => panic!("Unsuported expression: {:?} {} {:?}", lty, op, rty),
            }
        },

        Expression::Literal(Literal::Number(ref val)) => builder.build_const_f64(*val),
        Expression::Literal(Literal::String(ref val)) => {
            let val = {
                val.into_iter()
                    .fold(None, |prev, item| {
                        let item = match *item {
                            StringPart::String(ref s) => builder.build_const_string(s),
                            StringPart::Expression(ref e) => {
                                let exp = expression(e.clone(), scope, builder);
                                match exp.ty {
                                    TypeId::String => exp,
                                    TypeId::f64 => call_stl(
                                        builder,
                                        hct_atom!("to_string"),
                                        vec![ &exp ],
                                    ),
                                    _ => unimplemented!(),
                                }
                            },
                        };

                        Some(if let Some(prev) = prev {
                            call_stl(
                                builder,
                                hct_atom!("concat"),
                                vec![
                                    &prev,
                                    &item,
                                ],
                            )
                        } else {
                            item
                        })
                    })
            };

            val.unwrap_or_else(|| builder.build_const_string(String::new()))
        },
    }
}
