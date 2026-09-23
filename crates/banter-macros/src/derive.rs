//! `#[derive(FromArg)]` for enums of unit variants.

use unsynn::{IParse, Ident, Literal, ToTokenIter, ToTokens, TokenStream, quote};

use crate::{
    grammar::{Entry, Enum, Meta, Value, Variant},
    options::Invalid,
};

/// Spelling conventions for variant and type names.
struct Case;

impl Case {
    /// `FastMode` to `fast-mode`.
    fn kebab(name: &str) -> String {
        let mut out = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                out.push('-');
            }
            out.extend(c.to_lowercase());
        }
        out
    }
}

/// One accepted spelling and the variant it selects.
struct Choice {
    word: String,
    variant: Ident,
}

impl Choice {
    const ATTR: &str = "arg";

    /// Kebab-case of the variant, unless `#[arg(name = "...")]` says otherwise.
    fn read(variant: &Variant) -> Result<Self, Invalid> {
        let mut word = Case::kebab(&variant.name.to_string());
        for attr in &variant.attrs {
            let Meta::List(list) = attr.meta() else {
                continue;
            };
            if list.first != Self::ATTR {
                continue;
            }
            for entry in list.second.content {
                match entry.value {
                    Entry::Pair(pair) if pair.key == "name" => match pair.value {
                        Value::Text(text) => word = text.as_str().to_string(),
                        Value::Path(_) => return Err(Invalid("`name` takes a string".into())),
                    },
                    _ => return Err(Invalid("#[arg] only accepts `name = \"...\"`".into())),
                }
            }
        }
        Ok(Self {
            word,
            variant: variant.name.clone(),
        })
    }
}

pub struct Derive;

impl Derive {
    pub fn expand(item: TokenStream) -> TokenStream {
        let parsed: Enum = match item.to_token_iter().parse() {
            Ok(parsed) => parsed,
            Err(_) => return Invalid("FromArg can only be derived for enums".into()).emit(),
        };
        let choices: Result<Vec<Choice>, Invalid> = parsed
            .variants
            .content
            .iter()
            .map(|d| Choice::read(&d.value))
            .collect();
        let choices = match choices {
            Ok(choices) => choices,
            Err(err) => return err.emit(),
        };

        let name = &parsed.name;
        let kind = Literal::string(&Case::kebab(&name.to_string()));
        let listed = Literal::string(
            &choices
                .iter()
                .map(|c| c.word.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );
        let arms = choices.iter().map(|choice| {
            let word = Literal::string(&choice.word);
            let variant = &choice.variant;
            quote! { #word => ::core::result::Result::Ok(Self::#variant), }
        });

        quote! {
            impl ::banter::FromArg for #name {
                const KIND: &'static str = #kind;

                fn from_arg(raw: &str) -> ::core::result::Result<Self, ::std::string::String> {
                    match raw.to_ascii_lowercase().as_str() {
                        #{arms}
                        _ => ::core::result::Result::Err(
                            ::std::format!("expected one of {}", #listed)
                        ),
                    }
                }
            }
        }
    }
}
