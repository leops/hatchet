use std::ops::Deref;
use std::ffi::CStr;
use std::fmt::*;

use compiler::builder::{Type, Value};
use atom::Atom;

#[allow(non_camel_case_types)]
#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum TypeId {
    Void,
    f64,
    bool,
    i64,

    Context,
    Atom,
    Entity,
    String,

    Array {
        len: u32,
        ty: Box<TypeId>,
    },
    Vec {
        ty: Box<TypeId>,
    },

    Object {
        items: Vec<(Atom, TypeId)>,
    },
    /* TODO: HashMap {
        key: TypeId,
        value: TypeId,
    },*/

    Other(Type),
}

/// Wrapper for LLVM values with type metadata
#[derive(Clone)]
pub struct ValueRef {
    pub ty: TypeId,
    pub ptr: Value,
}

impl Debug for ValueRef {
    fn fmt(&self, fmt: &mut Formatter) -> Result {
        use llvm_sys::core::*;

        let mut ty_ptr = None;
        let ty_str = if let TypeId::Other(ptr) = self.ty {
            unsafe {
                let ptr = LLVMPrintTypeToString(ptr);
                ty_ptr = Some(ptr);

                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        } else {
            format!("{:?}", self.ty)
        };

        let val_ptr = unsafe {
            LLVMPrintValueToString(self.ptr)
        };
        let val_str = unsafe {
            CStr::from_ptr(val_ptr).to_string_lossy().into_owned()
        };

        let res = {
            fmt.debug_struct("ValueRef")
                .field("ty", &ty_str)
                .field("ptr", &val_str)
                .finish()
        };

        if let Some(ptr) = ty_ptr {
            unsafe {
                LLVMDisposeMessage(ptr);
            }
        }
        unsafe {
            LLVMDisposeMessage(val_ptr);
        }

        res
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum Global {
    Atom(Atom),
    String(String),
}

pub fn global_name<T: Debug, N: Deref<Target=str>>(ty: &T, name: &N) -> String {
    format!(
        "{:?}_{}", ty,
        name.chars()
            .filter(|chr| match *chr {
                'a'...'z' | 'A'...'Z' | '0'...'9' | '_' | '-' => true,
                _ => false,
            })
            .collect::<String>()
    )
}

impl From<Atom> for Global {
    fn from(value: Atom) -> Global {
        Global::Atom(value)
    }
}

impl From<String> for Global {
    fn from(value: String) -> Global {
        Global::String(value)
    }
}
