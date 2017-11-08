use std::collections::HashMap;
use std::borrow::Borrow;
use std::ops::Deref;
use std::ffi::CString;

use llvm_sys::*;
use llvm_sys::core::*;
use llvm_sys::prelude::*;

use atom::Atom;
use compiler::types::*;
use runtime::stl::Externals;
use runtime::types::Function;
use vmf::ir::Entity;

pub type Type = LLVMTypeRef;
pub type Value = LLVMValueRef;
pub type Block = LLVMBasicBlockRef;

#[derive(Debug)]
pub struct Builder {
    context: LLVMContextRef,
    module: LLVMModuleRef,
    func: Value,
    builder: LLVMBuilderRef,
    entities: HashMap<Atom, Entity>,
    globals: HashMap<Global, Value>,
    externals: Externals,
}

pub struct ModuleHolder {
    context: LLVMContextRef,
    pub ptr: LLVMModuleRef,
}

impl Drop for ModuleHolder {
    fn drop(&mut self) {
        unsafe {
            LLVMContextDispose(self.context);
        }
    }
}

pub type BuilderResult = (
    ModuleHolder,
    HashMap<Atom, Entity>,
    HashMap<Global, Value>,
    Externals,
);

impl Builder {
    pub fn new(name: &str, entities: HashMap<Atom, Entity>) -> Builder {
        let context = unsafe {
            LLVMContextCreate()
        };

        let module = {
            let mod_name = CString::new(name).unwrap();
            unsafe {
                LLVMModuleCreateWithNameInContext(mod_name.as_ptr(), context)
            }
        };

        let builder = unsafe {
            LLVMCreateBuilderInContext(context)
        };

        let main = {
            let i8_ty = unsafe { LLVMInt8TypeInContext(context) };
            let mut params = vec![unsafe {
                LLVMPointerType(
                    i8_ty,
                    0
                )
            }];
            let main_type = unsafe {
                LLVMFunctionType(
                    LLVMVoidTypeInContext(context),
                    params.as_mut_ptr(),
                    params.len() as u32,
                    0
                )
            };

            let func_name = CString::new("main").unwrap();
            unsafe {
                LLVMAddFunction(module, func_name.as_ptr(), main_type)
            }
        };

        let entry_name = CString::new("entry").unwrap();
        let entry_block = unsafe {
            LLVMAppendBasicBlockInContext(context, main, entry_name.as_ptr())
        };

        let start_name = CString::new("start").unwrap();
        let start_block = unsafe {
            LLVMAppendBasicBlockInContext(context, main, start_name.as_ptr())
        };

        unsafe {
            LLVMPositionBuilderAtEnd(builder, entry_block);
            LLVMBuildBr(builder, start_block);
            LLVMPositionBuilderAtEnd(builder, start_block)
        }

        Builder {
            context,
            module,
            builder,
            func: main,

            entities,
            globals: Default::default(),
            externals: Externals::new(),
        }
    }

    pub fn finalize(self) -> BuilderResult {
        unsafe {
            LLVMBuildRetVoid(self.builder);
            LLVMDisposeBuilder(self.builder);
        }

        (
            ModuleHolder {
                context: self.context,
                ptr: self.module,
            },
            self.entities,
            self.globals,
            self.externals,
        )
    }

    fn global_ptr<S, T>(&mut self, ty: TypeId, val: S) -> ValueRef where S: Borrow<T>, T: ToOwned + Deref<Target=str>, Global: From<<T as ToOwned>::Owned> {
        let cmp_ty = self.get_type(&ty);
        let &mut Builder { ref mut globals, module, .. } = self;

        let val = val.borrow();
        let glob = Global::from(val.to_owned());
        let &mut ptr = globals.entry(glob).or_insert_with(|| {
            let name = CString::new(global_name(&ty, val)).unwrap();
            unsafe {
                LLVMAddGlobalInAddressSpace(
                    module,
                    LLVMGetElementType(cmp_ty),
                    name.as_ptr(),
                    0
                )
            }
        });

        ValueRef { ty, ptr }
    }
}

/*impl Drop for Builder {
    fn drop(&mut self) {
        unsafe {
            LLVMDisposeBuilder(self.builder);
        }
    }
}*/

static EMPTY_STRING: [i8; 1] = [0];

macro_rules! builder_forward {
    ($name:ident ( $( $args:ident ),* ) -> $ret:tt = $op:ident ) => {
        pub fn $name(&mut self, $( $args: &ValueRef ),* ) -> ValueRef {
            ValueRef {
                ty: TypeId::$ret,
                ptr: unsafe {
                    $op(self.builder, $( $args.ptr, )* EMPTY_STRING.as_ptr())
                },
            }
        }
    };
}
macro_rules! builder_cmp {
    ($name:ident<f64, $pred:ident>( $( $args:ident ),* ) ) => {
        pub fn $name(&mut self, $( $args: &ValueRef ),* ) -> ValueRef {
            ValueRef {
                ty: TypeId::bool,
                ptr:  unsafe {
                    LLVMBuildFCmp(self.builder, LLVMRealPredicate::$pred, $( $args.ptr, )* EMPTY_STRING.as_ptr())
                },
            }
        }
    };
    ($name:ident<i64, $pred:ident>( $( $args:ident ),* ) ) => {
        pub fn $name(&mut self, $( $args: &ValueRef ),* ) -> ValueRef {
            ValueRef {
                ty: TypeId::bool,
                ptr:  unsafe {
                    LLVMBuildICmp(self.builder, LLVMIntPredicate::$pred, $( $args.ptr, )* EMPTY_STRING.as_ptr())
                },
            }
        }
    };
}

impl Builder {
    pub fn get_type(&self, ty: &TypeId) -> Type {
        match *ty {
            TypeId::Context | TypeId::Entity |
            TypeId::Atom | TypeId::String |
            TypeId::Vec { .. } => unsafe {
                LLVMPointerType(LLVMInt8TypeInContext(self.context), 0)
            },
            TypeId::Array { ref len, ref ty } => unsafe {
                LLVMPointerType(
                    LLVMArrayType(
                        self.get_type(ty),
                        *len as u32,
                    ),
                    0,
                )
            },
            TypeId::Object { ref items } => {
                let mut items: Vec<_> = {
                    items.iter()
                        .map(|&(_, ref ty)| self.get_type(ty))
                        .collect()
                };

                unsafe {
                    LLVMPointerType(
                        LLVMStructTypeInContext(
                            self.context,
                            items.as_mut_ptr(),
                            items.len() as _,
                            0,
                        ),
                        0,
                    )
                }
            },
            TypeId::f64 => unsafe { LLVMDoubleTypeInContext(self.context) },
            TypeId::bool => unsafe { LLVMInt1TypeInContext(self.context) },
            TypeId::i64 => unsafe { LLVMInt64TypeInContext(self.context) },
            TypeId::Void => unsafe { LLVMVoidTypeInContext(self.context) },
            TypeId::Other(ty) => ty,
        }
    }
    pub fn get_element_type(&self, ty: &TypeId) -> TypeId {
        TypeId::Other(unsafe {
            LLVMGetElementType(
                self.get_type(ty)
            )
        })
    }
    pub fn build_undef(&self, ty: &TypeId) -> ValueRef {
        ValueRef {
            ty: ty.clone(),
            ptr: unsafe {
                LLVMGetUndef(
                    self.get_type(ty)
                )
            },
        }
    }

    pub fn get_function_type<'a, A: IntoIterator<Item=&'a TypeId>>(&self, ret: TypeId, args: A) -> TypeId {
        let mut args: Vec<_> = {
            args.into_iter()
                .map(|ty| self.get_type(ty))
                .collect()
        };

        TypeId::Other(unsafe {
            LLVMFunctionType(
                self.get_type(&ret),
                args.as_mut_ptr(),
                args.len() as u32,
                0
            )
        })
    }
    pub fn add_function(&self, ty: TypeId, name: &str) -> ValueRef {
        let llvm_ty = self.get_type(&ty);
        let c_name = CString::new(name).unwrap();
        let ptr = unsafe {
            LLVMAddFunction(self.module, c_name.as_ptr(), llvm_ty)
        };

        ValueRef { ty, ptr }
    }
    pub fn add_function_attribute(&self, func: &ValueRef, attr: u32) {
        unsafe {
            let attr = LLVMCreateEnumAttribute(self.context, attr, 0);
            LLVMAddAttributeAtIndex(func.ptr, LLVMAttributeFunctionIndex, attr);
        }
    }

    pub fn get_external(&mut self, name: Atom) -> Function {
        self.externals.get_function(self, name)
    }
    pub fn get_runtime_context(&self) -> ValueRef {
        ValueRef {
            ty: TypeId::Context,
            ptr: unsafe {
                LLVMGetParam(self.func, 0)
            },
        }
    }
    pub fn build_call(&mut self, func: Function, args: Vec<ValueRef>) -> ValueRef {
        let mut args: Vec<_> = {
            args.into_iter()
                .map(|val| val.ptr)
                .collect()
        };

        let ptr = unsafe {
            LLVMBuildCall(
                self.builder,
                func.ptr,
                args.as_mut_ptr(),
                args.len() as u32,
                EMPTY_STRING.as_ptr()
            )
        };

        ValueRef { ty: func.ret, ptr }
    }

    pub fn add_entity(&mut self, name: Atom, ent: Entity) {
        self.entities.insert(name, ent);
    }
    pub fn add_auto_entity(&mut self) {
        self.entities
            .entry(hct_atom!(""))
            .or_insert(Entity {
                classname: hct_atom!("logic_auto"),
                .. Default::default()
            });
    }
    pub fn get_entities(&self) -> Vec<Atom> {
        self.entities.keys()
            .cloned()
            .collect()
    }

    pub fn build_const_f64(&self, val: f64) -> ValueRef {
        ValueRef {
            ty: TypeId::f64,
            ptr: unsafe {
                LLVMConstReal(self.get_type(&TypeId::f64), val)
            },
        }
    }
    pub fn build_const_i32(&self, val: i32) -> ValueRef {
        let ty = unsafe { LLVMInt32TypeInContext(self.context) };
        ValueRef {
            ty: TypeId::Other(ty),
            ptr: unsafe {
                LLVMConstInt(ty, val as u64, 1)
            },
        }
    }
    pub fn build_const_i64(&self, val: i64) -> ValueRef {
        ValueRef {
            ty: TypeId::i64,
            ptr: unsafe {
                LLVMConstInt(self.get_type(&TypeId::i64), val as u64, 1)
            },
        }
    }
    pub fn build_const_entity<A: Borrow<Atom>>(&mut self, val: A) -> ValueRef {
        self.global_ptr(TypeId::Entity, val)
    }
    pub fn build_const_atom<A: Borrow<Atom>>(&mut self, val: A) -> ValueRef {
        self.global_ptr(TypeId::Atom, val)
    }
    pub fn build_const_string<S: Borrow<String>>(&mut self, val: S) -> ValueRef {
        self.global_ptr(TypeId::String, val)
    }

    pub fn is_constant(&self, val: &ValueRef) -> bool {
        unsafe {
            LLVMIsConstant(val.ptr) == 1
        }
    }
    pub fn get_const_f64(&self, val: &ValueRef) -> Option<f64> {
        if val.ty == TypeId::f64 && self.is_constant(val) {
            let mut precision_lost = 0;
            let res = unsafe {
                LLVMConstRealGetDouble(val.ptr, &mut precision_lost)
            };

            if precision_lost == 0 {
                Some(res)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn append_basic_block(&mut self) -> Block {
        unsafe {
            LLVMAppendBasicBlockInContext(self.context, self.func, EMPTY_STRING.as_ptr())
        }
    }
    pub fn get_insert_block(&self) -> Block {
        unsafe {
            LLVMGetInsertBlock(self.builder)
        }
    }
    pub fn position_at_end(&mut self, block: Block) {
        unsafe {
            LLVMPositionBuilderAtEnd(self.builder, block);
        }
    }
    pub fn build_br(&mut self, block: Block) {
        unsafe {
            LLVMBuildBr(self.builder, block);
        }
    }
    pub fn build_cond_br(&mut self, cond: &ValueRef, cons: Block, alt: Block) {
        assert_eq!(cond.ty, TypeId::bool);
        unsafe {
            LLVMBuildCondBr(self.builder, cond.ptr, cons, alt);
        }
    }

    pub fn build_phi(&mut self, ty: TypeId) -> ValueRef {
        let llvm_ty = self.get_type(&ty);
        let ptr = unsafe {
            LLVMBuildPhi(self.builder, llvm_ty, EMPTY_STRING.as_ptr())
        };
        ValueRef { ty, ptr }
    }
    pub fn add_incoming(&self, phi: &ValueRef, mut val: ValueRef, mut block: Block) {
        unsafe {
            LLVMAddIncoming(
                phi.ptr,
                &mut val.ptr,
                &mut block,
                1,
            );
        }
    }

    pub fn build_alloca(&mut self, val: &TypeId) -> ValueRef {
        let current_block = unsafe {
            LLVMGetInsertBlock(self.builder)
        };

        unsafe {
            let alloca_block = LLVMGetEntryBasicBlock(self.func);
            let term = LLVMGetBasicBlockTerminator(alloca_block);
            LLVMPositionBuilderBefore(self.builder, term);
        }

        let ty = self.get_type(val);
        let ptr = unsafe {
            LLVMBuildAlloca(self.builder, ty, EMPTY_STRING.as_ptr())
        };

        unsafe {
            LLVMPositionBuilderAtEnd(self.builder, current_block);
        }

        ValueRef { ty: val.clone(), ptr }
    }
    pub fn build_load(&mut self, val: &ValueRef) -> ValueRef {
        let ptr = unsafe {
            LLVMBuildLoad(self.builder, val.ptr, EMPTY_STRING.as_ptr())
        };

        ValueRef { ty: val.ty.clone(), ptr }
    }
    pub fn build_store(&mut self, val: &ValueRef, ptr: &ValueRef) {
        unsafe {
            LLVMBuildStore(self.builder, val.ptr, ptr.ptr);
        }
    }
    pub fn build_in_bounds_gep<'a, I: IntoIterator<Item=&'a ValueRef>>(&mut self, obj: &ValueRef, indices: I) -> ValueRef {
        let mut indices: Vec<_> = {
            indices.into_iter()
                .map(|i| i.ptr)
                .collect()
        };

        let ptr = unsafe {
            LLVMBuildInBoundsGEP(
                self.builder,
                obj.ptr,
                indices.as_mut_ptr(),
                indices.len() as u32,
                EMPTY_STRING.as_ptr()
            )
        };

        ValueRef { ty: obj.ty.clone(), ptr }
    }
    pub fn build_insert_value(&mut self, container: &ValueRef, elem: &ValueRef, index: u32) -> ValueRef {
        let ptr = unsafe {
            LLVMBuildInsertValue(
                self.builder,
                container.ptr,
                elem.ptr,
                index,
                EMPTY_STRING.as_ptr()
            )
        };

        ValueRef { ty: container.ty.clone(), ptr }
    }

    builder_forward!{ build_fadd(rhs, lhs) -> f64 = LLVMBuildFAdd }
    builder_forward!{ build_fsub(rhs, lhs) -> f64 = LLVMBuildFSub }
    builder_forward!{ build_fmul(rhs, lhs) -> f64 = LLVMBuildFMul }
    builder_forward!{ build_fdiv(rhs, lhs) -> f64 = LLVMBuildFDiv }
    builder_forward!{ build_frem(rhs, lhs) -> f64 = LLVMBuildFRem }

    builder_forward!{ build_nswadd(rhs, lhs) -> i64 = LLVMBuildNSWAdd }
    builder_forward!{ build_shl(rhs, lhs) -> i64 = LLVMBuildShl }
    builder_forward!{ build_lshr(rhs, lhs) -> i64 = LLVMBuildLShr }

    builder_cmp!{ build_float_lt<f64, LLVMRealOLT>(rhs, lhs) }
    builder_cmp!{ build_float_le<f64, LLVMRealOLE>(rhs, lhs) }
    builder_cmp!{ build_float_gt<f64, LLVMRealOGT>(rhs, lhs) }
    builder_cmp!{ build_float_ge<f64, LLVMRealOGE>(rhs, lhs) }
    builder_cmp!{ build_float_eq<f64, LLVMRealOEQ>(rhs, lhs) }
    builder_cmp!{ build_float_ne<f64, LLVMRealONE>(rhs, lhs) }

    builder_forward!{ build_and(rhs, lhs) -> bool = LLVMBuildAnd }
    builder_forward!{ build_or(rhs, lhs) -> bool = LLVMBuildOr }
    builder_forward!{ build_xor(rhs, lhs) -> bool = LLVMBuildXor }
    builder_forward!{ build_not(val) -> bool = LLVMBuildNot }

    builder_cmp!{ build_int_lt<i64, LLVMIntULT>(rhs, lhs) }
    builder_cmp!{ build_int_eq<i64, LLVMIntEQ>(rhs, lhs) }
    builder_cmp!{ build_int_ne<i64, LLVMIntNE>(rhs, lhs) }
}