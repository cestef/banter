//! What `#[command(...)]` may say about a method.

use unsynn::{Literal, ToTokens, TokenStream, quote};

use crate::grammar::{Attr, Entry, Meta, Value};

/// One recognised key.
enum Key {
    Name,
    Alias,
    Group,
    Tag,
    Hidden,
    Skip,
}

impl Key {
    fn parse(word: &str) -> Option<Self> {
        Some(match word {
            "name" => Self::Name,
            "alias" => Self::Alias,
            "group" => Self::Group,
            "tag" => Self::Tag,
            "hidden" => Self::Hidden,
            "skip" => Self::Skip,
            _ => return None,
        })
    }
}

/// Everything the attributes on one method configure.
#[derive(Default)]
pub struct Options {
    pub name: Option<String>,
    pub aliases: Vec<String>,
    pub group: Option<String>,
    pub tags: Vec<TokenStream>,
    pub hidden: bool,
    pub skip: bool,
    /// Doc comment lines, trimmed.
    pub docs: Vec<String>,
}

/// A problem with an attribute, reported at the call site.
pub struct Invalid(pub String);

impl Options {
    pub const ATTR: &str = "command";

    /// Read every `#[doc]` and `#[command]` attribute.
    pub fn read(attrs: &[Attr]) -> Result<Self, Invalid> {
        let mut options = Self::default();
        for attr in attrs {
            match attr.meta() {
                Meta::Pair(pair) if pair.key == "doc" => {
                    if let Value::Text(text) = pair.value {
                        options.docs.push(text.as_str().trim().to_string());
                    }
                }
                Meta::List(list) if list.first == Self::ATTR => {
                    for entry in list.second.content {
                        options.apply(entry.value)?;
                    }
                }
                Meta::Word(word) if word == Self::ATTR => {}
                _ => {}
            }
        }
        Ok(options)
    }

    /// Whether an attribute is ours and must be stripped from the output.
    pub fn owns(attr: &Attr) -> bool {
        attr.meta().name().as_deref() == Some(Self::ATTR)
    }

    /// The first paragraph of the doc comment, joined into one line.
    pub fn help(&self) -> String {
        self.docs
            .iter()
            .skip_while(|l| l.is_empty())
            .take_while(|l| !l.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn apply(&mut self, entry: Entry) -> Result<(), Invalid> {
        let (word, value) = match entry {
            Entry::Flag(word) => (word.to_string(), None),
            Entry::Pair(pair) => (pair.key.to_string(), Some(pair.value)),
        };
        let key =
            Key::parse(&word).ok_or_else(|| Invalid(format!("unknown command option `{word}`")))?;
        let text = |value: Option<Value>| match value {
            Some(Value::Text(text)) => Ok(text.as_str().to_string()),
            _ => Err(Invalid(format!("`{word}` takes a string"))),
        };
        match key {
            Key::Name => self.name = Some(text(value)?),
            Key::Alias => self.aliases.push(text(value)?),
            Key::Group => self.group = Some(text(value)?),
            Key::Hidden => self.hidden = true,
            Key::Skip => self.skip = true,
            Key::Tag => match value {
                Some(Value::Path(path)) => self.tags.push(path.to_token_stream()),
                _ => return Err(Invalid("`tag` takes a type".into())),
            },
        }
        Ok(())
    }
}

impl Invalid {
    /// `compile_error!("...")`.
    pub fn emit(self) -> TokenStream {
        let message = Literal::string(&self.0);
        quote! { ::core::compile_error!(#message); }
    }
}
