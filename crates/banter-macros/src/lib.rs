//! Procedural macros for [banter](https://docs.rs/banter). Use them through the
//! `banter` crate, which re-exports both.

mod commands;
mod derive;
mod grammar;
mod options;

use proc_macro::TokenStream;

/// Turn every method of an `impl` block into a command of that type's state.
///
/// - `&self` or `&mut self` is the session state.
/// - A `&mut Session` parameter receives the session.
/// - Every other parameter is an argument: `T` required, `Option<T>` optional,
///   `Vec<T>` whatever is left. Any `FromArg` type works.
/// - The first paragraph of the doc comment is the help text.
/// - `#[command(name = "..", alias = "..", group = "..", tag = Type, hidden, skip)]`
///   adjusts the rest.
///
/// Methods must return `banter::Result<T>` where `T` is `()` or `Flow`. Associated
/// functions without a receiver are left alone.
#[proc_macro_attribute]
pub fn commands(_attr: TokenStream, item: TokenStream) -> TokenStream {
    commands::Table::expand(item.into()).into()
}

/// `FromArg` for an enum of unit variants, matched case-insensitively by kebab-case
/// name, or by `#[arg(name = "...")]`.
#[proc_macro_derive(FromArg, attributes(arg))]
pub fn from_arg(item: TokenStream) -> TokenStream {
    derive::Derive::expand(item.into()).into()
}
