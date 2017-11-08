macro_rules! decl_type {
    ($gen:tt, (ref T)) => {
        decl_type!($gen, $gen)
    };
    ($gen:tt, T) => {
        decl_type!($gen, $gen)
    };
    ($gen:tt, (Vec<T>)) => {
        decl_type!($gen, (Vec<$gen>))
    };
    ($gen:tt, (mut Vec<T>)) => {
        decl_type!($gen, (mut Vec<$gen>))
    };

    ($gen:tt, Atom) => {
        *const Atom
    };
    ($gen:tt, Entity) => {
        decl_type!($gen, Atom)
    };
    ($gen:tt, ($kw:tt Entity)) => {
        decl_type!($gen, Atom)
    };

    ($gen:tt, (ref String)) => {
        decl_type!($gen, String)
    };
    ($gen:tt, String) => {
        *const String
    };

    ($gen:tt, (mut Vec<$ty:tt>)) => {
        *mut Vec<decl_type!($gen, $ty)>
    };
    ($gen:tt, (Vec<$ty:tt>)) => {
        *const Vec<decl_type!($gen, $ty)>
    };

    ($gen:tt, Context) => {
        *mut Context
    };
    ($gen:tt, f64) => {
        f64
    };
    ($gen:tt, bool) => {
        bool
    };
    ($gen:tt, i64) => {
        i64
    };
    ($gen:tt, Void) => {
        ()
    };
}

macro_rules! check_ptr {
    /*($name:ident, $method:tt, $ty:tt) => {
        if let Some($name) = unsafe { $name.$method() } {
            $name
        } else {
            error!(concat!(stringify!($ty), " pointer for ", stringify!($name), " is null"));
            panic!()
        }
    };*/
    ($name:ident, $method:tt, $ty:tt) => {
        unsafe { ::unreachable::UncheckedOptionExt::unchecked_unwrap($name.$method()) }
    };
}

macro_rules! find_context {
    ($name:ident Context $( $rest:tt )* ) => {
        $name
    };
    ($name:ident $type:tt $( $rest:tt )* ) => {
        find_context!( $( $rest )* )
    };
    () => { () };
}

macro_rules! convert_arg {
    ($gen:tt, $name:ident, Context, $ctx:expr) => {
        check_ptr!($name, as_mut, Context)
    };
    ($gen:tt, $name:ident, Atom, $ctx:expr) => {{
        check_ptr!($name, as_ref, Atom)
    }};

    ($gen:tt, $name:ident, Entity, $ctx:expr) => {
        convert_arg!($gen, $name, Atom, $ctx)
    };
    ($gen:tt, $name:ident, (ref Entity), $ctx:expr) => {{
        let $name = convert_arg!($gen, $name, Atom, $ctx);
        $ctx.entities.get($name).expect(&format!("entity \"{}\" not found", $name))
    }};
    ($gen:tt, $name:ident, (mut Entity), $ctx:expr) => {{
        let $name = convert_arg!($gen, $name, Atom, $ctx);
        $ctx.entities.get_mut($name).expect(&format!("entity \"{}\" not found", $name))
    }};

    ($gen:tt, $name:ident, (Vec<T>), $ctx:expr) => {
        convert_arg!($gen, $name, (Vec<$gen>), $ctx)
    };
    ($gen:tt, $name:ident, (mut Vec<T>), $ctx:expr) => {
        convert_arg!($gen, $name, (mut Vec<$gen>), $ctx)
    };
    ($gen:tt, $name:ident, (Vec<$ty:tt>), $ctx:expr) => {
        check_ptr!($name, as_ref, Vec)
    };
    ($gen:tt, $name:ident, (mut Vec<$ty:tt>), $ctx:expr) => {
        check_ptr!($name, as_mut, Vec)
    };

    ($gen:tt, $name:ident, T, $ctx:expr) => {
        convert_arg!($gen, $name, $gen, $ctx)
    };
    ($gen:tt, $name:ident, (mut T), $ctx:expr) => {
        convert_arg!($gen, $name, (mut $gen), $ctx)
    };

    ($gen:tt, $name:ident, String, $ctx:expr) => {
        check_ptr!($name, as_ref, String)
    };
    ($gen:tt, $name:ident, $ty:tt, $ctx:expr) => {
        $name
    };
}

macro_rules! convert_res {
    ($gen:tt, $name:block, $ctx:expr, Atom) => {
        $ctx.arenas.atoms.alloc($name) as decl_type!($gen, Atom)
    };
    ($gen:tt, $name:block, $ctx:expr, Entity) => {
        convert_res!($gen, $name, $ctx, Atom)
    };
    ($gen:tt, $name:block, $ctx:expr, ($kw:tt Entity)) => {
        convert_res!($gen, $name, $ctx, Atom)
    };

    ($gen:tt, $name:block, $ctx:expr, (ref String)) => {
        $name as decl_type!($gen, String)
    };
    ($gen:tt, $name:block, $ctx:expr, String) => {
        $ctx.arenas.strings.alloc($name) as decl_type!($gen, String)
    };

    ($gen:tt, $name:block, $ctx:expr, (Vec<Entity>)) => {
        $ctx.arenas.ent_vec.alloc($name) as decl_type!($gen, (Vec<Entity>))
    };

    ($gen:tt, $name:block, $ctx:expr, (ref T)) => {
        $name
    };
    ($gen:tt, $name:block, $ctx:expr, T) => {
        convert_res!($gen, $name, $ctx, $gen)
    };

    ($gen:tt, $name:block, $ctx:expr, $ty:tt) => {
        $name
    };
}

macro_rules! convert_args {
    ( $gen:tt, $ctx:expr; $arg:ident $type:tt $( $rest:tt )* ) => {
        let $arg = convert_arg!($gen, $arg, $type, $ctx);
        convert_args!( $gen, $ctx; $( $rest )* );
    };
    ( $gen:tt, $ctx:expr; ) => {};
}

macro_rules! type_id {
    ($gen:tt, T) => {
        type_id!($gen, $gen)
    };
    ($gen:tt, (Vec<T>)) => {
        TypeId::Vec {
            ty: box type_id!($gen, $gen),
        }
    };
    ($gen:tt, (mut Vec<T>)) => {
        type_id!($gen, (Vec<$gen>))
    };

    ($gen:tt, (Vec<$ty:tt>)) => {
        TypeId::Vec {
            ty: box type_id!($gen, $ty),
        }
    };
    ($gen:tt, (mut Vec<$ty:tt>)) => {
        type_id!($gen, (Vec<$ty>))
    };

    ($gen:tt, (mut $ty:tt)) => {
        type_id!($gen, $ty)
    };
    ($gen:tt, (ref $ty:tt)) => {
        type_id!($gen, $ty)
    };
    ($gen:tt, $ty:tt) => {
        TypeId::$ty
    };
}

macro_rules! attr_kind {
    (argmemonly) => (4);
    (nonnull) => (31);
    (readnone) => (34);
    (readonly) => (35);
}

macro_rules! concat_str {
    ($id:expr, ()) => ( $id );
    ($id:expr, $gen:tt) => ( concat!($id, ".", stringify!($gen)) );
}

macro_rules! insert_func {
    ( $gen:tt; ( $builder:ident, $m_name:ident ); $name:ident; $id:expr; $ret:tt; $( $attr:ident ),* ; $( $args:tt ),* ) => {
        if $m_name == hct_atom_function!($gen, $name) {
            let fn_type = $builder.get_function_type(
                type_id!($gen, $ret),
                &[
                    $( type_id!($gen, $args) ),*
                ],
            );

            let func = $builder.add_function(fn_type, concat_str!($id, $gen));

            $builder.add_function_attribute(&func, attr_kind!(argmemonly));
            $( $builder.add_function_attribute(&func, attr_kind!($attr)); )*

            return Function {
                ptr: func.ptr,
                args: vec![
                    $( type_id!($gen, $args) ),*
                ],
                ret: type_id!($gen, $ret),
            };
        }
    };
}

macro_rules! for_each_type {
    ( T; $submac:ident ! { $( $args:tt )* } ) => {
        $submac!{ f64; $( $args )* }
        $submac!{ bool; $( $args )* }
        $submac!{ i64; $( $args )* }

        $submac!{ Atom; $( $args )* }
        $submac!{ Entity; $( $args )* }
        $submac!{ String; $( $args )* }
    };

    ( (T: Eq); $submac:ident ! { $( $args:tt )* } ) => {
        $submac!{ Atom; $( $args )* }
        $submac!{ Entity; $( $args )* }
        $submac!{ String; $( $args )* }
    };
}

macro_rules! build_externals {
    ($ctx:tt; intrinsic!( $name:ident = $id:tt ( $( $args:tt ),* ) -> $ret:tt ); $( $tail:tt )* ) => {
        insert_func!{ (); $ctx; $name; $id; $ret; ; $( $args ),* }
        build_externals!{ $ctx; $( $tail )* }
    };
    ($ctx:tt; intrinsic!( $name:ident = $id:tt ( $( $args:tt ),* ) ); $( $tail:tt )* ) => {
        insert_func!{ (); $ctx; $name; $id; Void; ; $( $args ),* }
        build_externals!{ $ctx; $( $tail )* }
    };

    ($ctx:tt;  $( #[ $attr:ident ] )* fn $name:ident ( $( $args:ident : $type:tt ),* ) -> $ret:tt $body:block $( $tail:tt )* ) => {
        insert_func!{ (); $ctx; $name; concat!("hatchet.", stringify!($name)); $ret; $( $attr ),* ; $( $type ),* }
        build_externals!{ $ctx; $( $tail )* }
    };
    ($ctx:tt;  $( #[ $attr:ident ] )* fn $name:ident ( $( $args:ident : $type:tt ),* ) $body:block $( $tail:tt )* ) => {
        insert_func!{ (); $ctx; $name; concat!("hatchet.", stringify!($name)); Void; $( $attr ),* ; $( $type ),* }
        build_externals!{ $ctx; $( $tail )* }
    };
    ($ctx:tt;  $( #[ $attr:ident ] )* fn $name:ident < $gen:tt > ( $( $args:ident : $type:tt ),* ) -> $ret:tt $body:block $( $tail:tt )* ) => {
        for_each_type! { $gen; insert_func!{ $ctx; $name; concat!("hatchet.", stringify!($name)); $ret; $( $attr ),* ; $( $type ),* } }
        build_externals!{ $ctx; $( $tail )* }
    };
    ($ctx:tt;  $( #[ $attr:ident ] )* fn $name:ident < $gen:tt > ( $( $args:ident : $type:tt ),* ) $body:block $( $tail:tt )* ) => {
        for_each_type! { $gen; insert_func!{ $ctx; $name; concat!("hatchet.", stringify!($name)); Void; $( $attr ),* ; $( $type ),* } }
        build_externals!{ $ctx; $( $tail )* }
    };

    ($ctx:tt; ) => {};
}

macro_rules! add_symbol {
    ($gen:tt; $self:ident; $name:ident; $ret:tt; $body:block; $( $args:ident : $type:tt ),* ) => {
        if $self.functions.borrow().contains_key(&hct_atom_function!($gen, $name)) {
            trace!(concat!("Register function ", stringify!($name)));

            let func: fn( $( decl_type!($gen, $type) ),* ) -> decl_type!($gen, $ret) = {
                |$( $args : decl_type!($gen, $type) ),*| -> decl_type!($gen, $ret) {
                    convert_args!( $gen, find_context!( $( $args $type )* ); $( $args $type )* );
                    convert_res!($gen, $body, find_context!( $( $args $type )* ), $ret)
                }
            };

            unsafe {
                ::llvm_sys::support::LLVMAddSymbol(
                    CString::new(concat_str!(concat!("hatchet.", stringify!($name)), $gen)).unwrap().as_ptr(),
                    func as *mut _,
                );
            }
        }
    };
}

macro_rules! register_symbols {
    ($self:ident; intrinsic!( $name:ident = $id:tt ( $( $args:tt ),* ) -> $ret:tt ); $( $tail:tt )* ) => {
        register_symbols!($self; $( $tail )* );
    };
    ($self:ident; intrinsic!( $name:ident = $id:tt ( $( $args:tt ),* ) ); $( $tail:tt )* ) => {
        register_symbols!($self; $( $tail )* );
    };

    ($self:ident;  $( #[ $attr:ident ] )* fn $name:ident ( $( $args:ident : $type:tt ),* ) -> $ret:tt $body:block $( $tail:tt )* ) => {
        add_symbol!((); $self; $name; $ret; $body; $( $args : $type ),* );
        register_symbols!($self; $( $tail )* );
    };
    ($self:ident;  $( #[ $attr:ident ] )* fn $name:ident ( $( $args:ident : $type:tt ),* ) $body:block $( $tail:tt )* ) => {
        add_symbol!((); $self; $name; Void; $body; $( $args : $type ),* );
        register_symbols!($self; $( $tail )* );
    };
    ($self:ident;  $( #[ $attr:ident ] )* fn $name:ident < $gen:tt > ( $( $args:ident : $type:tt ),* ) -> $ret:tt $body:block $( $tail:tt )* ) => {
        for_each_type! { $gen; add_symbol!{ $self; $name; $ret; $body; $( $args : $type ),* } }
        register_symbols!($self; $( $tail )* );
    };
    ($self:ident;  $( #[ $attr:ident ] )* fn $name:ident < $gen:tt > ( $( $args:ident : $type:tt ),* ) $body:block $( $tail:tt )* ) => {
        for_each_type! { $gen; add_symbol!{ $self; $name; Void; $body; $( $args : $type ),* } }
        register_symbols!($self; $( $tail )* );
    };

    ($self:ident; ) => {};
}

/// Declare STL functions through the Externals structs
#[macro_export]
macro_rules! declare_externals {
    ( $( $body:tt )* ) => {
        #[derive(Default, Debug, PartialEq)]
        pub struct Externals {
            pub functions: RefCell<HashMap<Atom, Function>>,
        }

        impl Externals {
            #[cfg_attr(feature="clippy", allow(new_without_default_derive))]
            pub fn new() -> Externals {
                Externals {
                    functions: RefCell::new(HashMap::new()),
                }
            }

            /// Get an external function by name, adding it to the context if needed
            pub fn get_function(&self, builder: &Builder, name: Atom) -> Function {
                self.functions.borrow_mut()
                    .entry(name.clone())
                    .or_insert_with(|| {
                        build_externals!{ (builder, name); $( $body )* }
                        panic!("unknown function {:?}", name.to_string())
                    })
                    .clone()
            }

            /// Add the corresponding symbols to the runtime for all the registered functions
            pub fn register_symbols(&self) {
                register_symbols!(self; $( $body )* );
            }
        }
    };
}
