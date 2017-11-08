use std::collections::HashMap;
use std::fmt::*;
use libc::c_void;

use llvm_sys::LLVMModule;
use llvm_sys::execution_engine::*;
use llvm_sys::prelude::*;
use llvm_sys::core::*;
use rand::{Rng, SeedableRng, StdRng};
use typed_arena::Arena;

use compiler::builder::*;
use compiler::types::{TypeId, Global};
use vmf::ir::{Script, Entity};
use atom::Atom;

/// Pointer and type metadata for a function
#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub ptr: Value,
    pub args: Vec<TypeId>,
    pub ret: TypeId,
}

pub struct Arenas {
    pub atoms: Arena<Atom>,
    pub strings: Arena<String>,
    pub ent_vec: Arena<Vec<*const Atom>>
}

impl Arenas {
    #[cfg_attr(feature="clippy", allow(not_unsafe_ptr_arg_deref))]
    pub fn with_globals(module: *mut LLVMModule, engine: LLVMExecutionEngineRef, globals: HashMap<Global, LLVMValueRef>) -> Arenas {
        let atoms = Arena::new();
        let strings = Arena::new();

        let globals: HashMap<_, _> = {
            globals.into_iter()
                .map(|(k, v)| (v, k))
                .collect()
        };

        let mut global = unsafe { LLVMGetFirstGlobal(module) };
        while !global.is_null() {
            let val = &globals[&global];
            trace!("Register global {:?}", val);

            let val = match *val {
                Global::Atom(ref val) => atoms.alloc(val.clone()) as *mut _ as *mut c_void,
                Global::String(ref val) => strings.alloc(val.clone()) as *mut _ as *mut c_void,
            };

            unsafe {
                LLVMAddGlobalMapping(engine, global, val);
            }

            global = unsafe { LLVMGetNextGlobal(global) };
        }

        Arenas {
            atoms, strings,
            ent_vec: Arena::new(),
        }
    }
}

/// Execution context of a script
pub struct Context {
    pub entities: HashMap<Atom, Entity>,
    pub arenas: Arenas,
    pub rng: Box<Rng>,
}

impl Context {
    pub fn new(ent: Script, arenas: Arenas, entities: HashMap<Atom, Entity>) -> Context {
        Context {
            arenas, entities,
            rng: box StdRng::from_seed(&[ent.seed]),
        }
    }
}

impl Debug for Context {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        fmt.debug_struct("Context")
            .field("entities", &self.entities)
            .finish()
    }
}
