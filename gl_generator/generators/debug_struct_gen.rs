// Copyright 2013-2014 The gl-rs developers. For a full listing of the authors,
// refer to the AUTHORS file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use registry::{Registry, Ns};
use std::io;
use std::collections::HashMap;

#[allow(missing_copy_implementations)]
pub struct DebugStructGenerator;

impl super::Generator for DebugStructGenerator {
    fn write<W>(&self, registry: &Registry, ns: Ns, dest: &mut W) -> io::Result<()> where W: io::Write {
        try!(write_header(dest));
        try!(write_type_aliases(&ns, dest));
        try!(write_enums(registry, dest));
        try!(write_fnptr_struct_def(dest));
        try!(write_panicking_fns(&ns, dest));

        // allows the overriding of some functions
        let mut fn_overrides = HashMap::new();
        fn_overrides.insert("glDebugMessageCallback", ("fallback_debug_message_callback", "debug_output_fallback_required"));
        fn_overrides.insert("glDebugMessageInsert", ("fallback_debug_message_insert", "debug_output_fallback_required"));
        fn_overrides.insert("glGetError", ("fallback_get_error", "debug_output_fallback_required"));

        try!(write_struct(registry, &ns, &fn_overrides, dest));
        try!(write_impl(registry, &ns, &fn_overrides, dest));
        Ok(())
    }
}

/// Creates a `__gl_imports` module which contains all the external symbols that we need for the
///  bindings.
fn write_header<W>(dest: &mut W) -> io::Result<()> where W: io::Write {
    writeln!(dest, r#"
        mod __gl_imports {{
            extern crate gl_common;
            extern crate libc;
            pub use std::mem;
            pub use std::marker::Send;
            pub use std::cell::RefCell;
            pub use std::ptr::null_mut;
            pub use std::ffi::CString;
        }}
    "#)
}

/// Creates a `types` module which contains all the type aliases.
///
/// See also `generators::gen_type_aliases`.
fn write_type_aliases<W>(ns: &Ns, dest: &mut W) -> io::Result<()> where W: io::Write {
    try!(writeln!(dest, r#"
        pub mod types {{
            #![allow(non_camel_case_types)]
            #![allow(non_snake_case)]
            #![allow(dead_code)]
            #![allow(missing_copy_implementations)]
    "#));

    try!(super::gen_type_aliases(ns, dest));

    writeln!(dest, "}}")
}

/// Creates all the `<enum>` elements at the root of the bindings.
fn write_enums<W>(registry: &Registry, dest: &mut W) -> io::Result<()> where W: io::Write {
    for e in registry.enum_iter() {
        try!(super::gen_enum_item(e, "types::", dest));
    }

    Ok(())
}

/// Creates a `FnPtr` structure which contains the store for a single binding.
fn write_fnptr_struct_def<W>(dest: &mut W) -> io::Result<()> where W: io::Write {
    writeln!(dest, "
        #[allow(dead_code)]
        #[allow(missing_copy_implementations)]
        #[allow(raw_pointer_derive)]
        #[derive(Clone, Debug)]
        pub struct FnPtr {{
            /// The function pointer that will be used when calling the function.
            f: *const __gl_imports::libc::c_void,
            /// True if the pointer points to a real function, false if points to a `panic!` fn.
            is_loaded: bool,
        }}

        #[allow(dead_code)]
        #[allow(raw_pointer_derive)]
        #[allow(missing_copy_implementations)]
        #[derive(Clone, Debug)]
        pub enum OverridableFnPtr {{
            Loaded(*const __gl_imports::libc::c_void),
            Overridden(*const __gl_imports::libc::c_void, *const __gl_imports::libc::c_void)
        }}

        impl OverridableFnPtr {{
            pub fn get_original(&self) -> *const __gl_imports::libc::c_void {{
                match *self {{
                    OverridableFnPtr::Loaded(ptr) => ptr,
                    OverridableFnPtr::Overridden(original, _) => original
                }}
            }}
        }}

        impl FnPtr {{
            /// Creates a `FnPtr` from a load attempt.
            fn new(ptr: *const __gl_imports::libc::c_void) -> Self {{
                if ptr.is_null() {{
                    FnPtr {{
                        f: missing_fn_panic as *const __gl_imports::libc::c_void,
                        is_loaded: false
                    }}
                }} else {{
                    FnPtr {{
                        f: ptr,
                        is_loaded: true
                    }}
                }}
            }}

            /// Returns `true` if the function has been successfully loaded.
            ///
            /// If it returns `false`, calling the corresponding function will fail.
            #[inline]
            #[allow(dead_code)]
            pub fn is_loaded(&self) -> bool {{
                self.is_loaded
            }}
        }}
    ")
}

/// Creates a `panicking` module which contains one function per GL command.
///
/// These functions are the mocks that are called if the real function could not be loaded.
fn write_panicking_fns<W>(ns: &Ns, dest: &mut W) -> io::Result<()> where W: io::Write {
    writeln!(dest,
        "#[inline(never)]
        fn missing_fn_panic() -> ! {{
            panic!(\"{ns} function was not loaded\")
        }}",
        ns = ns
    )
}

/// Creates a structure which stores all the `FnPtr` of the bindings.
///
/// The name of the struct corresponds to the namespace.
fn write_struct<W>(registry: &Registry, ns: &Ns, fn_overrides: &HashMap<&str, (&str, &str)>, dest: &mut W) -> io::Result<()> where W: io::Write {
    try!(dest.write(include_str!("debug_output/header.rs").as_bytes()));

    try!(writeln!(dest, "
        #[allow(non_camel_case_types)]
        #[allow(non_snake_case)]
        #[allow(dead_code)]
        pub struct {ns} {{
            trace_callback: Box<Fn(&str, &str, &str)>,
            debug_output: __gl_imports::RefCell<DebugOutputState>,",
        ns = ns.fmt_struct_name()
    ));

    for c in registry.cmd_iter() {
        let symbol = super::gen_symbol_name(ns, &c.proto.ident);

        if let Some(v) = registry.aliases.get(&c.proto.ident) {
            try!(writeln!(dest, "/// Fallbacks: {}", v.join(", ")));
        }
        if fn_overrides.contains_key(&*symbol) {
            try!(writeln!(dest,
                "pub {name}: OverridableFnPtr,",
                name = c.proto.ident
            ));
        } else {
            try!(writeln!(dest,
                "pub {name}: FnPtr,",
                name = c.proto.ident
            ));
        }
    }

    writeln!(dest, "}}")
}

/// Creates the `impl` of the structure created by `write_struct`.
fn write_impl<W>(registry: &Registry, ns: &Ns, fn_overrides: &HashMap<&str, (&str, &str)>, dest: &mut W) -> io::Result<()> where W: io::Write {
    try!(writeln!(dest,
        "impl {ns} {{",
        ns = ns.fmt_struct_name()
    ));

    try!(dest.write(include_str!("debug_output/impl.rs").as_bytes()));

    try!(writeln!(dest, "
            /// Load each OpenGL symbol using a custom load function. This allows for the
            /// use of functions like `glfwGetProcAddress` or `SDL_GL_GetProcAddress`.
            ///
            /// ~~~ignore
            /// let gl = Gl::load_with(|s| glfw.get_proc_address(s));
            /// ~~~
            #[allow(dead_code)]
            #[allow(unused_variables)]
            pub fn load_with<F>(mut loadfn: F, trace_callback: Box<Fn(&str, &str, &str)>) -> {ns} where F: FnMut(&str) -> *const __gl_imports::libc::c_void {{
                let mut metaloadfn = |symbol: &str, symbols: &[&str]| {{
                    let mut ptr = loadfn(symbol);
                    if ptr.is_null() {{
                        for &sym in symbols.iter() {{
                            ptr = loadfn(sym);
                            if !ptr.is_null() {{ break; }}
                        }}
                    }}
                    ptr
                }};

                let debug_output_fallback_required = !metaloadfn(\"glDebugMessageCallback\", &[\"glDebugMessageCallbackARB\", \"glDebugMessageCallbackKHR\"]).is_null();

                {ns} {{
                    trace_callback: trace_callback,
                    debug_output: __gl_imports::RefCell::new(DebugOutputState {{
                        enabled: true,
                        callback: None,
                        user_param: __gl_imports::null_mut(),
                        last_error: NO_ERROR
                    }}),",
        ns = ns.fmt_struct_name()
    ));

    for c in registry.cmd_iter() {
        let symbol = super::gen_symbol_name(ns, &c.proto.ident);

        let load = format!(
            "metaloadfn(\"{symbol}\", &[{fallbacks}])",
            symbol = symbol,
            fallbacks = match registry.aliases.get(&c.proto.ident) {
                Some(fbs) => {
                    fbs.iter()
                       .map(|name| format!("\"{}\"", super::gen_symbol_name(ns, &name)))
                       .collect::<Vec<_>>().join(", ")
                },
                None => format!(""),
            }
        );

        match fn_overrides.get(&*symbol) {
            Some(&(fn_override, condition)) => {
                let typed_params = super::gen_parameters(c, false, true);
                let return_suffix = super::gen_return_type(c);
                let override_params = typed_params_to_override_params(ns.fmt_struct_name(), typed_params, &return_suffix);

                try!(writeln!(
                    dest,
                    "{name}: if {condition} {{ OverridableFnPtr::Loaded({load}) }} else {{ let override_fn: *const extern \"system\" fn({override_params}) -> {return_suffix} = {struct_name}::{fn_override} as *const _; OverridableFnPtr::Overridden({load}, override_fn as *const __gl_imports::libc::c_void) }},",
                    name = c.proto.ident,
                    struct_name = ns.fmt_struct_name(),
                    fn_override = fn_override,
                    load = load,
                    override_params = override_params.join(", "),
                    return_suffix = return_suffix,
                    condition = condition
                ))
            },
            None => try!(writeln!(
                dest,
                "{name}: FnPtr::new({load}),",
                name = c.proto.ident,
                load = load
            ))
        };
    }

    try!(writeln!(dest,
            "}}
        }}

        /// Load each OpenGL symbol using a custom load function.
        ///
        /// ~~~ignore
        /// let gl = Gl::load(&glfw);
        /// ~~~
        #[allow(dead_code)]
        #[allow(unused_variables)]
        pub fn load<T: __gl_imports::gl_common::GlFunctionsSource>(loader: &T, trace_callback: Box<Fn(&str, &str, &str)>) -> {ns} {{
            {ns}::load_with(|name| loader.get_proc_addr(name), trace_callback)
        }}",
        ns = ns.fmt_struct_name()
    ));

    for c in registry.cmd_iter() {
        let symbol = super::gen_symbol_name(ns, &c.proto.ident);
        let idents = super::gen_parameters(c, true, false);
        let typed_params = super::gen_parameters(c, false, true);
        let return_suffix = super::gen_return_type(c);
        let println = format!("(self.trace_callback)(\"{ident}\", &format!(\"{params}\"{args}), &format!(\"{{:?}}\", r));",
                                ident = c.proto.ident,
                                params = (0 .. idents.len()).map(|_| "{:?}".to_string()).collect::<Vec<_>>().join(", "),
                                args = idents.iter().zip(typed_params.iter())
                                      .map(|(name, ty)| {
                                          if ty.contains("GLDEBUGPROC") {
                                              format!(", \"<callback>\"")
                                          } else {
                                              format!(", {}", name)
                                          }
                                      }).collect::<Vec<_>>().concat());

        let call = match fn_overrides.get(&*symbol) {
            Some(_) => {
                let connected_typed_params = typed_params.join(", ");
                let override_params = typed_params_to_override_params(ns.fmt_struct_name(), typed_params, &return_suffix);
                format!(
                    "match self.{name} {{
                        OverridableFnPtr::Overridden(original, new) => __gl_imports::mem::transmute::<_, extern \"system\" fn({override_params}) -> {return_suffix}>(new)(&self, &__gl_imports::mem::transmute::<_, extern \"system\" fn({typed_params}) -> {return_suffix}>(original), {idents}),
                        OverridableFnPtr::Loaded(original) => __gl_imports::mem::transmute::<_, extern \"system\" fn({typed_params}) -> {return_suffix}>(original)({idents})
                    }}",
                    name = c.proto.ident,
                    override_params = override_params.join(", "),
                    typed_params = connected_typed_params,
                    return_suffix = return_suffix,
                    idents = idents.join(", ")
                )
            },
            None => {
                format!(
                    "__gl_imports::mem::transmute::<_, extern \"system\" fn({typed_params}) -> {return_suffix}>\
                    (self.{name}.f)({idents})",
                    name = c.proto.ident,
                    typed_params = typed_params.join(", "),
                    return_suffix = return_suffix,
                    idents = idents.join(", ")
                )
            }
        };

        try!(writeln!(dest,
            "#[allow(non_snake_case)] #[allow(unused_variables)] #[allow(dead_code)]
            #[inline] pub unsafe fn {name}(&self, {params}) -> {return_suffix} {{ \
                let r = {call};
                {println}
                self.on_fn_called(\"{full_name}\");
                r
            }}",
            name = c.proto.ident,
            full_name = symbol,
            params = super::gen_parameters(c, true, true).join(", "),
            return_suffix = super::gen_return_type(c),
            call = call,
            println = println
        ))
    }

    writeln!(dest,
        "}}

        unsafe impl __gl_imports::Send for {ns} {{}}",
        ns = ns.fmt_struct_name()
    )
}

fn typed_params_to_override_params(struct_name: &str, typed_params: Vec<String>, return_suffix: &str) -> Vec<String> {
    let mut override_params = vec!(
        format!("&{}", struct_name),
        format!("&extern \"system\" fn({typed_params}) -> {return_suffix}", typed_params = typed_params.join(", "), return_suffix = return_suffix)
    );
    for param in typed_params {
        override_params.push(param);
    }
    override_params
}
