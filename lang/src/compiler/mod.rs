pub mod builder;
pub mod expression;
pub mod function;
pub mod path;
pub mod scope;
pub mod statements;
pub mod types;

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::mem;

use llvm_sys::core::*;
use llvm_sys::prelude::*;
use llvm_sys::execution_engine::*;
use llvm_sys::transforms::pass_manager_builder::*;
use llvm_sys::target::*;
use log::LogLevel::Trace;

use atom::Atom;
use logging::*;
use hct::ast;
use vmf::ir;
use vmf::*;

use self::builder::*;
use self::scope::*;
use self::statements::*;
use self::types::*;
use runtime::types::*;
use runtime::stl::Externals;

fn codegen(name: &str, script: ast::Script, entities: HashMap<Atom, Entity>) -> BuilderResult {
    let mut builder = Builder::new(name, entities);

    let scope = Scope::root(&mut builder);
    statements(
        script.body,
        scope,
        &mut builder,
    );

    builder.finalize()
}

fn optimize(module: LLVMModuleRef) {
    unsafe {
        let pm = LLVMCreatePassManager();

        let builder = LLVMPassManagerBuilderCreate();
        LLVMPassManagerBuilderSetOptLevel(builder, 2);
        LLVMPassManagerBuilderPopulateFunctionPassManager(builder, pm);
        LLVMPassManagerBuilderPopulateModulePassManager(builder, pm);
        LLVMPassManagerBuilderDispose(builder);

        LLVMRunPassManager(pm, module);
    }
}

type LinkResult = (LLVMExecutionEngineRef, extern "C" fn(&mut Context) -> (), Context);
fn link(ent: ir::Script, module: LLVMModuleRef, entities: HashMap<Atom, Entity>, globals: HashMap<Global, LLVMValueRef>, externals: &Externals) -> LinkResult {
    unsafe {
        LLVMLinkInMCJIT();
        LLVM_InitializeNativeTarget();
        LLVM_InitializeNativeAsmPrinter();
    }

    // Register external functions
    externals.register_symbols();

    let engine = unsafe {
        let mut engine = mem::uninitialized();
        let mut out = mem::zeroed();

        let res = LLVMCreateExecutionEngineForModule(&mut engine, module, &mut out);
        if res == 0 {
            engine
        } else {
            panic!()
        }
    };

    // Map globals to arena memory
    let arenas = Arenas::with_globals(module, engine, globals);

    let fname_s = CString::new("main").unwrap();
    let main_fn = unsafe {
        let addr = LLVMGetFunctionAddress(engine, fname_s.as_ptr());
        if addr == 0 {
            panic!()
        } else {
            mem::transmute(addr)
        }
    };

    let ctx = Context::new(ent, arenas, entities);
    (engine, main_fn, ctx)
}

fn module_to_string(module: LLVMModuleRef) -> String {
    unsafe {
        let c_str = LLVMPrintModuleToString(module);
        let val = CStr::from_ptr(c_str).to_string_lossy().into_owned();
        LLVMDisposeMessage(c_str);
        val
    }
}

/// Compiles and run a script on a map
pub fn apply(ent: ir::Script, script: ast::Script, entities: HashMap<Atom, Entity>) -> HashMap<Atom, Entity> {
    let codegen_start = timer_start!();

    let (module, entities, globals, externals) = codegen(&ent.script, script, entities);

    let opt_start = timer_chain!(codegen_start, time, "Codegen time: {}", time);

    let pre_opt: Option<String> = if log_enabled!(Trace) {
        let pre_opt = module_to_string(module.ptr);
        trace!("Codegen output:\n{}", pre_opt);

        Some(pre_opt)
    } else {
        None
    };

    optimize(module.ptr);

    if let Some(pre_opt) = pre_opt {
        trace!("Optimizer output:");
        print_diff(&pre_opt, &module_to_string(module.ptr));
    }

    let link_start = timer_chain!(opt_start, time, "Optimization time: {}", time);

    let (_ee, main_fn, mut ctx) = link(ent, module.ptr, entities, globals, &externals);

    let exec_start = timer_chain!(link_start, time, "Linking time: {}", time);

    main_fn(&mut ctx);

    timer_end!(exec_start, time, "Execution time: {}", time);

    ctx.entities
}
