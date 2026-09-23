//! `#[banter::commands]`: every method of an impl block becomes a command.

use unsynn::{IParse, Ident, Literal, Span, ToTokenIter, ToTokens, TokenStream, quote};

use crate::{
    grammar::{Impl, Item, Method, Ty, Typed},
    options::{Invalid, Options},
};

/// How many values an argument takes, mirroring `banter::Kind`.
enum Kind {
    Required,
    Optional,
    Rest,
}

impl Kind {
    /// `Option<T>` is optional, `Vec<T>` takes the rest, anything else is required.
    fn of(ty: &Ty) -> Self {
        match ty.head().as_deref() {
            Some("Option") => Self::Optional,
            Some("Vec") => Self::Rest,
            _ => Self::Required,
        }
    }

    fn variant(&self) -> Ident {
        let name = match self {
            Self::Required => "Required",
            Self::Optional => "Optional",
            Self::Rest => "Rest",
        };
        Ident::new(name, Span::call_site())
    }

    /// The `Ctx` accessor that reads this kind.
    fn accessor(&self) -> Ident {
        let name = match self {
            Self::Required => "arg",
            Self::Optional => "opt",
            Self::Rest => "all",
        };
        Ident::new(name, Span::call_site())
    }
}

/// One method parameter after the receiver.
enum Input {
    /// `&mut Session`, handed the session itself.
    Session,
    /// A typed argument parsed from the line.
    Arg { name: String, ty: Ty, kind: Kind },
}

impl Input {
    fn read(typed: &Typed) -> Self {
        if typed.ty.mentions("Session") {
            return Self::Session;
        }
        Self::Arg {
            name: typed.name.to_string().trim_start_matches("r#").to_string(),
            kind: Kind::of(&typed.ty),
            ty: typed.ty.clone(),
        }
    }
}

/// One method, read as a command.
struct Handler {
    method: Ident,
    options: Options,
    inputs: Vec<Input>,
}

impl Handler {
    /// `None` for associated functions and methods marked `skip`.
    fn read(method: &Method) -> Result<Option<Self>, Invalid> {
        let options = Options::read(&method.attrs)?;
        let params = &method.params.content;
        if params.receiver.is_none() || options.skip {
            return Ok(None);
        }
        Ok(Some(Self {
            method: method.name.clone(),
            inputs: params.args.iter().map(|d| Input::read(&d.value)).collect(),
            options,
        }))
    }

    fn name(&self) -> String {
        self.options
            .name
            .clone()
            .unwrap_or_else(|| self.method.to_string().trim_start_matches("r#").to_string())
    }

    fn args(&self) -> impl Iterator<Item = (&String, &Ty, &Kind)> {
        self.inputs.iter().filter_map(|input| match input {
            Input::Arg { name, ty, kind } => Some((name, ty, kind)),
            Input::Session => None,
        })
    }

    /// `::banter::Spec::new(..).alias(..).param(..)...`
    fn spec(&self) -> TokenStream {
        let name = Literal::string(&self.name());
        let help = Literal::string(&self.options.help());
        let aliases = self.options.aliases.iter().map(|alias| {
            let alias = Literal::string(alias);
            quote! { .alias(#alias) }
        });
        let params = self.args().map(|(name, _, kind)| {
            let name = Literal::string(name);
            let kind = kind.variant();
            quote! { .param(#name, ::banter::Kind::#kind) }
        });
        let group = self.options.group.iter().map(|group| {
            let group = Literal::string(group);
            quote! { .group(#group) }
        });
        let tags = self
            .options
            .tags
            .iter()
            .map(|tag| quote! { .tag::<#tag>() });
        let hidden = self.options.hidden.then(|| quote! { .hidden() });
        quote! {
            ::banter::Spec::new(#name)
                #{aliases}
                #{params}
                .help(#help)
                #{group}
                #{tags}
                #{hidden}
        }
    }

    /// A closure parsing each argument, then calling the method on the state.
    fn handler(&self) -> TokenStream {
        let method = &self.method;
        let slot = |i: usize| Ident::new(&format!("arg{i}"), Span::mixed_site());
        let reads = self.args().enumerate().map(|(i, (_, ty, kind))| {
            let slot = slot(i);
            let accessor = kind.accessor();
            let index = Literal::usize_unsuffixed(i);
            quote! { let #slot: #ty = ctx.#accessor(#index)?; }
        });
        let mut next = 0;
        let passed = self.inputs.iter().map(|input| match input {
            Input::Session => quote! { &mut *ctx.session, },
            Input::Arg { .. } => {
                let slot = slot(next);
                next += 1;
                quote! { #slot, }
            }
        });
        let passed: Vec<TokenStream> = passed.collect();
        quote! {
            |ctx: &mut ::banter::Ctx<'_, Self>| {
                #{reads}
                ctx.state.#method(#{passed})
            }
        }
    }

    fn command(&self) -> TokenStream {
        let spec = self.spec();
        let handler = self.handler();
        quote! { ::banter::Command::new(#spec, #handler), }
    }
}

/// The macro: strip `#[command]` attributes, emit the block, then its table.
pub struct Table;

impl Table {
    pub fn expand(item: TokenStream) -> TokenStream {
        let mut block: Impl = match item.to_token_iter().parse() {
            Ok(block) => block,
            Err(err) => return Invalid(format!("#[commands] expects an impl block: {err}")).emit(),
        };
        let handlers = match Self::handlers(&block) {
            Ok(handlers) => handlers,
            Err(err) => return err.emit(),
        };
        for item in &mut block.items.content {
            if let Item::Method(method) = item {
                method.attrs.retain(|a| !Options::owns(a));
            }
        }

        let ty = &block.ty;
        let commands = handlers.iter().map(Handler::command);
        let mut out = block.to_token_stream();
        out.extend(quote! {
            impl ::banter::Commands for #ty {
                fn commands() -> ::std::vec::Vec<::banter::Command<Self>> {
                    ::std::vec![#{commands}]
                }
            }
        });
        out
    }

    fn handlers(block: &Impl) -> Result<Vec<Handler>, Invalid> {
        let mut handlers = Vec::new();
        for item in &block.items.content {
            if let Item::Method(method) = item
                && let Some(handler) = Handler::read(method)?
            {
                handlers.push(handler);
            }
        }
        Ok(handlers)
    }
}
