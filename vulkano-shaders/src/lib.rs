//! The procedural macro for vulkano's shader system.
//! Manages the compile-time compilation of GLSL into SPIR-V and generation of assosciated rust code.
//!
//! # Basic usage
//!
//! ```
//! #[macro_use]
//! extern crate vulkano_shaders;
//! extern crate vulkano;
//! # fn main() {}
//! vulkano_shader!{
//!     mod_name: vertex_shader,
//!     ty: "vertex",
//!     src: "
//! #version 450
//!
//! layout(location = 0) in vec3 position;
//!
//! void main() {
//!     gl_Position = vec4(position, 1.0);
//! }"
//! }
//! ```
//!
//! # Details
//!
//! If you want to take a look at what the macro generates, your best options
//! are to either read through the code that handles the generation (the
//! [`reflect`][reflect] function in the `vulkano-shaders` crate) or use a tool
//! such as [cargo-expand][cargo-expand] to view the expansion of the macro in your
//! own code. It is unfortunately not possible to provide a `generated_example`
//! module like some normal macro crates do since derive macros cannot be used from
//! the crate they are declared in. On the other hand, if you are looking for a
//! high-level overview, you can see the below section.
//! 
//! # Generated code overview
//! 
//! The macro generates the following items of interest:
//! * The `Shader` struct. This contains a single field, `shader`, which is an
//! `Arc<ShaderModule>`.
//! * The `Shader::load` constructor. This method takes an `Arc<Device>`, calls
//! [`ShaderModule::new`][ShaderModule::new] with the passed-in device and the
//! shader data provided via the macro, and returns `Result<Shader, OomError>`.
//! Before doing so, it loops through every capability instruction in the shader
//! data, verifying that the passed-in `Device` has the appropriate features
//! enabled. **This function currently panics if a feature required by the shader
//! is not enabled on the device.** At some point in the future it will return
//! an error instead.
//! * The `Shader::module` method. This method simply returns a reference to the
//! `Arc<ShaderModule>` contained within the `shader` field of the `Shader`
//! struct.
//! * Methods for each entry point of the shader module. These construct and
//! return the various entry point structs that can be found in the
//! [vulkano::pipeline::shader][pipeline::shader] module.
//! * A Rust struct translated from each struct contained in the shader data.
//! * The `Layout` newtype. This contains a [`ShaderStages`][ShaderStages] struct.
//! An implementation of [`PipelineLayoutDesc`][PipelineLayoutDesc] is also
//! generated for the newtype.
//! * The `SpecializationConstants` struct. This contains a field for every
//! specialization constant found in the shader data. Implementations of
//! `Default` and [`SpecializationConstants`][SpecializationConstants] are also
//! generated for the struct.
//! 
//! All of these generated items will be accessed through the module specified
//! by `mod_name: foo` If you wanted to store the `Shader` in a struct of your own,
//! you could do something like this:
//! 
//! ```
//! # #[macro_use]
//! # extern crate vulkano_shader_derive;
//! # extern crate vulkano;
//! # fn main() {}
//! # use std::sync::Arc;
//! # use vulkano::OomError;
//! # use vulkano::device::Device;
//! #
//! # vulkano_shader!{
//! #     mod_name: vertex_shader,
//! #     ty: "vertex",
//! #     src: "
//! # #version 450
//! #
//! # layout(location = 0) in vec3 position;
//! #
//! # void main() {
//! #     gl_Position = vec4(position, 1.0);
//! # }"
//! # }
//! // various use statements
//! // `vertex_shader` module with shader derive
//! 
//! pub struct Shaders {
//!     pub vertex_shader: vertex_shader::Shader
//! }
//! 
//! impl Shaders {
//!     pub fn load(device: Arc<Device>) -> Result<Self, OomError> {
//!         Ok(Self {
//!             vertex_shader: vertex_shader::Shader::load(device)?,
//!         })
//!     }
//! }
//! ```
//! 
//! # Options
//! 
//! The options available are in the form of the following attributes:
//!
//! ## `mod_name: ...`
//!
//! Specifies the identifier for the module that the generated code goes in.
//!
//! ## `ty: "..."`
//!
//! This defines what shader type the given GLSL source will be compiled into.
//! The type can be any of the following:
//!
//! * `vertex`
//! * `fragment`
//! * `geometry`
//! * `tess_ctrl`
//! * `tess_eval`
//! * `compute`
//!
//! For details on what these shader types mean, [see Vulkano's documentation][pipeline].
//!
//! ## `src: "..."`
//!
//! Provides the raw GLSL source to be compiled in the form of a string. Cannot
//! be used in conjunction with the `path` field.
//!
//! ## `path: "..."`
//!
//! Provides the path to the GLSL source to be compiled, relative to `Cargo.toml`.
//! Cannot be used in conjunction with the `src` field.
//!
//! ## `dump: true`
//!
//! The crate fails to compile but prints the generated rust code to stdout.
//! 
//! [reflect]: https://github.com/vulkano-rs/vulkano/blob/master/vulkano-shaders/src/lib.rs#L67
//! [cargo-expand]: https://github.com/dtolnay/cargo-expand
//! [ShaderModule::new]: https://docs.rs/vulkano/*/vulkano/pipeline/shader/struct.ShaderModule.html#method.new
//! [OomError]: https://docs.rs/vulkano/*/vulkano/enum.OomError.html
//! [pipeline::shader]: https://docs.rs/vulkano/*/vulkano/pipeline/shader/index.html
//! [descriptor]: https://docs.rs/vulkano/*/vulkano/descriptor/index.html
//! [ShaderStages]: https://docs.rs/vulkano/*/vulkano/descriptor/descriptor/struct.ShaderStages.html
//! [PipelineLayoutDesc]: https://docs.rs/vulkano/*/vulkano/descriptor/pipeline_layout/trait.PipelineLayoutDesc.html
//! [SpecializationConstants]: https://docs.rs/vulkano/*/vulkano/pipeline/shader/trait.SpecializationConstants.html
//! [pipeline]: https://docs.rs/vulkano/*/vulkano/pipeline/index.html

#![doc(html_logo_url = "https://raw.githubusercontent.com/vulkano-rs/vulkano/master/logo.png")]

#![recursion_limit = "1024"]
#[macro_use] extern crate quote;
             extern crate shaderc;
             extern crate proc_macro;
             extern crate proc_macro2;
#[macro_use] extern crate syn;


use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use syn::parse::{Parse, ParseStream, Result};
use syn::{Ident, LitStr, LitBool};

mod codegen;
mod descriptor_sets;
mod entry_point;
mod enums;
mod parse;
mod spec_consts;
mod structs;
mod spirv_search;

use codegen::ShaderKind;

enum SourceKind {
    Src(String),
    Path(String),
}

struct MacroInput {
    mod_ident: Ident,
    shader_kind: ShaderKind,
    source_kind: SourceKind,
    dump: bool,
}

impl Parse for MacroInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut dump = None;
        let mut mod_ident = None;
        let mut shader_kind = None;
        let mut source_kind = None;

        while !input.is_empty() {
            let name: Ident = input.parse()?;
            input.parse::<Token![:]>()?;

            match name.to_string().as_ref() {
                "mod_name" => {
                    if mod_ident.is_some() {
                        panic!("Only one `mod` can be defined")
                    }

                    let mod_name: Ident = input.parse()?;
                    mod_ident = Some(mod_name);
                }
                "ty" => {
                    if shader_kind.is_some() {
                        panic!("Only one `ty` can be defined")
                    }

                    let ty: LitStr = input.parse()?;
                    let ty = match ty.value().as_ref() {
                        "vertex" => ShaderKind::Vertex,
                        "fragment" => ShaderKind::Fragment,
                        "geometry" => ShaderKind::Geometry,
                        "tess_ctrl" => ShaderKind::TessControl,
                        "tess_eval" => ShaderKind::TessEvaluation,
                        "compute" => ShaderKind::Compute,
                        _ => panic!("Unexpected shader type, valid values: vertex, fragment, geometry, tess_ctrl, tess_eval, compute")
                    };
                    shader_kind = Some(ty);
                }
                "src" => {
                    if source_kind.is_some() {
                        panic!("Only one `src` or `path` can be defined")
                    }

                    let src: LitStr = input.parse()?;
                    source_kind = Some(SourceKind::Src(src.value()));
                }
                "path" => {
                    if source_kind.is_some() {
                        panic!("Only one `src` or `path` can be defined")
                    }

                    let path: LitStr = input.parse()?;
                    source_kind = Some(SourceKind::Path(path.value()));
                }
                "dump" => {
                    if dump.is_some() {
                        panic!("Only one `dump` can be defined")
                    }
                    let dump_lit: LitBool = input.parse()?;
                    dump = Some(dump_lit.value);
                }
                name => panic!(format!("Unknown field name: {}", name))
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let shader_kind = match shader_kind {
            Some(shader_kind) => shader_kind,
            None => panic!("Please provide a shader type e.g. `ty: \"vertex\"`")
        };

        let source_kind = match source_kind {
            Some(source_kind) => source_kind,
            None => panic!("Please provide a source e.g. `path: \"foo.glsl\"` or `src: \"glsl source code here ...\"`")
        };

        let mod_ident = match mod_ident {
            Some(mod_ident) => mod_ident,
            None => panic!("Please provide a mod e.g. `mod: fs` ")
        };

        let dump = dump.unwrap_or(false);

        Ok(MacroInput { shader_kind, source_kind, mod_ident, dump })
    }
}

#[proc_macro]
pub fn vulkano_shader(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as MacroInput);

    let source_code = match input.source_kind {
        SourceKind::Src(source) => source,
        SourceKind::Path(path) => {
            let root = env::var("CARGO_MANIFEST_DIR").unwrap_or(".".into());
            let full_path = Path::new(&root).join(&path);

            if full_path.is_file() {
                let mut buf = String::new();
                File::open(full_path)
                    .and_then(|mut file| file.read_to_string(&mut buf))
                    .expect(&format!("Error reading source from {:?}", path));
                buf
            } else {
                panic!("File {:?} was not found ; note that the path must be relative to your Cargo.toml", path);
            }
        }
    };

    let content = codegen::compile(&source_code, input.shader_kind).unwrap();
    codegen::reflect("Shader", content.as_binary(), &input.mod_ident, input.dump).unwrap().into()
}
