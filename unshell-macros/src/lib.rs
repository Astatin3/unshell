//! Proc macros for `unshell` application-layer leaf declarations.

mod leaf_decl;
mod procedure;
mod procedures;
mod utils;

use proc_macro::TokenStream;
use syn::{DeriveInput, ItemImpl, ItemStruct, parse_macro_input};

/// Declares one compile-time leaf surface and binds it to endpoint and/or TUI
/// host structs.
///
/// What it is: an attribute macro placed on a marker struct that generates the
/// shared protocol-visible metadata for one leaf and applies that metadata to the
/// listed host structs.
///
/// Why it exists: endpoint and TUI hosts should not each have to repeat the leaf
/// name and procedure inventory, and endpoint construction should not need a
/// handwritten list of procedure ids.
///
/// # Example
/// ```ignore
/// #[unshell::leaf(
///     name = "remote_shell",
///     procedures = [Open],
///     leaf_endpoint = endpoint::RemoteShellEndpoint,
///     leaf_tui = tui::RemoteShellTui,
/// )]
/// pub struct RemoteShell;
/// ```
#[proc_macro_attribute]
pub fn leaf(attr: TokenStream, item: TokenStream) -> TokenStream {
    match leaf_decl::expand_leaf_declaration(
        parse_macro_input!(attr as leaf_decl::LeafDeclarationAttributes),
        parse_macro_input!(item as ItemStruct),
    ) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Derives canonical stateful-procedure metadata for one procedure type.
///
/// What it is: a derive macro that records one procedure suffix and generates
/// the canonical `protocol_procedure_id()` helper for that procedure.
///
/// Why it exists: hook-backed procedures need one stable `procedure_id`, but the
/// runtime should not require each procedure to handwrite the identifier logic.
///
/// # Example
/// ```ignore
/// use unshell::{Procedure, leaf};
///
/// #[leaf(
///     name = "shell",
///     procedures = [OpenSession],
///     endpoint_struct = ShellLeaf,
/// )]
/// struct Shell;
///
/// struct ShellLeaf;
///
/// #[derive(Procedure)]
/// #[procedure(leaf = ShellLeaf, name = "open")]
/// struct OpenSession;
///
/// assert!(OpenSession::protocol_procedure_id().ends_with(".open"));
/// ```
#[proc_macro_derive(Procedure, attributes(procedure))]
pub fn derive_procedure(input: TokenStream) -> TokenStream {
    match procedure::expand_procedure(parse_macro_input!(input as DeriveInput)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

/// Generates dispatch glue for a simple call-driven leaf impl block.
///
/// What it is: an attribute macro placed on one `impl` block whose `#[call]`
/// methods define the callable surface for that leaf.
///
/// Why it exists: one-shot leaves should be able to declare a small RPC-like API
/// on ordinary Rust methods while still producing the canonical procedure list
/// and dispatch logic expected by the protocol runtime.
///
/// # Example
/// ```ignore
/// use unshell::{leaf, procedures};
///
/// #[leaf(
///     id = "org.example.v1.echo",
///     procedures = ["echo"],
///     endpoint_struct = EchoLeaf,
/// )]
/// struct Echo;
///
/// struct EchoLeaf;
///
/// #[procedures(error = core::convert::Infallible)]
/// impl EchoLeaf {
///     #[call]
///     fn echo(&mut self, input: String) -> String {
///         input
///     }
/// }
///
/// assert!(EchoLeaf::protocol_procedure_id("echo").is_some());
/// ```
#[proc_macro_attribute]
pub fn procedures(attr: TokenStream, item: TokenStream) -> TokenStream {
    match procedures::expand_procedures(
        parse_macro_input!(attr as procedures::ProceduresAttributes),
        parse_macro_input!(item as ItemImpl),
    ) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}
