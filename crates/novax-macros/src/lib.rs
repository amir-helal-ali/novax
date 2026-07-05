//! NovaX Procedural Macros
//!
//! Provides #[novax::main], #[novax::route], and other macros for zero-boilerplate development.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, ItemStruct, parse_macro_input};

/// Mark a function as the NovaX application entrypoint.
/// Wraps the function in `novax_runtime::block_on`.
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let fn_name = &input.sig.ident;
    let fn_block = &input.block;

    let expanded = quote! {
        fn #fn_name() {
            novax_runtime::block_on(async {
                #fn_block
            });
        }

        fn main() {
            novax_observability::init_logging("info");
            tracing::info!("NovaX application starting");
            #fn_name();
        }
    };

    expanded.into()
}

/// Mark a struct as a NovaX entity.
/// In v0.1 this is a marker macro. Future versions will generate
/// DB schema, migrations, ORM methods.
#[proc_macro_attribute]
pub fn entity(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    // For v0.1: just pass through unchanged
    // Future: parse attrs, generate impl block with find/insert/update/delete
    quote!(#input).into()
}

/// Mark a function as an HTTP route handler.
/// In v0.1 this is a marker macro. Future versions will generate
/// routing registration and OpenAPI specs.
#[proc_macro_attribute]
pub fn route(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    // For v0.1: just pass through unchanged
    quote!(#input).into()
}
