//! The slice of Rust syntax the macros read, as unsynn parsers.

// unsynn parsers return its own large error type by design.
#![allow(clippy::result_large_err)]

use unsynn::*;

unsynn! {
    keyword KwImpl = "impl";
    keyword KwFn = "fn";
    keyword KwSelf = "self";
    keyword KwMut = "mut";
    keyword KwPub = "pub";
    keyword KwEnum = "enum";

    /// `#[...]`
    pub struct Attr {
        pub pound: Pound,
        pub body: BracketGroup,
    }

    /// `pub`, `pub(crate)`, ...
    pub struct Vis {
        pub kw: KwPub,
        pub scope: Option<ParenthesisGroup>,
    }

    /// `self`, `&self`, `&mut self`, `mut self`.
    pub struct Receiver {
        pub amp: Option<And>,
        pub mutable: Option<KwMut>,
        pub kw: KwSelf,
    }

    /// `name: Type`, optionally `mut name: Type`.
    pub struct Typed {
        pub mutable: Option<KwMut>,
        pub name: Ident,
        pub colon: Colon,
        pub ty: Ty,
    }

    /// Everything between a method's parentheses.
    pub struct Params {
        pub receiver: Option<Cons<Receiver, Option<Comma>>>,
        pub args: CommaDelimitedVec<Typed>,
    }

    pub struct Method {
        pub attrs: Vec<Attr>,
        pub vis: Option<Vis>,
        pub kw: KwFn,
        pub name: Ident,
        pub params: ParenthesisGroupContaining<Params>,
        pub ret: Option<Cons<RArrow, Ty>>,
        pub body: BraceGroup,
    }

    /// Anything else in an impl block (consts, types, generic or `async` methods),
    /// kept verbatim: tokens up to a `;` or a body.
    pub struct Other {
        pub tokens: TokenStreamUntil<Either<Semicolon, BraceGroup>>,
        pub end: Either<Semicolon, BraceGroup>,
    }

    pub enum Item {
        Method(Method),
        Other(Other),
    }

    /// `impl Type { ... }`
    pub struct Impl {
        pub attrs: Vec<Attr>,
        pub kw: KwImpl,
        pub ty: Ty,
        pub items: BraceGroupContaining<Vec<Item>>,
    }

    /// `Name` or `Name = 3` inside an enum.
    pub struct Variant {
        pub attrs: Vec<Attr>,
        pub name: Ident,
        pub discriminant: Option<Cons<Assign, Ty>>,
    }

    /// `enum Name { ... }` with unit variants.
    pub struct Enum {
        pub attrs: Vec<Attr>,
        pub vis: Option<Vis>,
        pub kw: KwEnum,
        pub name: Ident,
        pub variants: BraceGroupContaining<CommaDelimitedVec<Variant>>,
    }

    /// `name = "value"` or `name = Path` inside an attribute.
    pub struct Pair {
        pub key: Ident,
        pub eq: Assign,
        pub value: Value,
    }

    pub enum Value {
        Text(LiteralString),
        Path(Ty),
    }

    /// One entry of `#[command(...)]`: a pair or a bare flag.
    pub enum Entry {
        Pair(Pair),
        Flag(Ident),
    }

    /// `name(entries)` or `name = "value"`, the inside of an attribute.
    pub enum Meta {
        List(Cons<Ident, ParenthesisGroupContaining<CommaDelimitedVec<Entry>>>),
        Pair(Pair),
        Word(Ident),
        Raw(TokenStream),
    }
}

/// A type, read up to a top-level comma, `=`-free brace, or the end. Tracks angle
/// brackets so `HashMap<K, V>` stays whole, and skips the `>` of `->`.
#[derive(Clone, Debug)]
pub struct Ty(pub TokenStream);

impl Parser for Ty {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let mut out = TokenStream::new();
        let mut depth = 0usize;
        let mut arrow = false;
        loop {
            let mut ahead = tokens.clone();
            let Some(tree) = ahead.next() else { break };
            let stop = match &tree {
                TokenTree::Punct(p) => match p.as_char() {
                    ',' | ';' | '=' => depth == 0,
                    _ => false,
                },
                TokenTree::Group(g) => depth == 0 && g.delimiter() == Delimiter::Brace,
                _ => false,
            };
            if stop {
                break;
            }
            if let TokenTree::Punct(p) = &tree {
                match p.as_char() {
                    '<' => depth += 1,
                    '>' if !arrow => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
            arrow = matches!(&tree, TokenTree::Punct(p) if p.as_char() == '-' && p.spacing() == Spacing::Joint);
            tokens.next();
            out.extend([tree]);
        }
        if out.is_empty() {
            return Error::other(None, tokens, "expected a type".into());
        }
        Ok(Self(out))
    }
}

impl ToTokens for Ty {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.0.clone());
    }
}

impl Ty {
    /// The first identifier, e.g. `Option` for `Option<u64>`.
    pub fn head(&self) -> Option<String> {
        self.0.clone().into_iter().find_map(|t| match t {
            TokenTree::Ident(i) => Some(i.to_string()),
            _ => None,
        })
    }

    /// Whether any identifier in the type is `name`.
    pub fn mentions(&self, name: &str) -> bool {
        self.0
            .clone()
            .into_iter()
            .any(|t| matches!(t, TokenTree::Ident(i) if i == name))
    }
}

impl Attr {
    /// The attribute's contents, classified.
    pub fn meta(&self) -> Meta {
        let stream = self.body.0.stream();
        let mut iter = stream.clone().to_token_iter();
        Meta::parse_all(&mut iter).unwrap_or(Meta::Raw(stream))
    }
}

impl Meta {
    pub fn name(&self) -> Option<String> {
        match self {
            Self::List(list) => Some(list.first.to_string()),
            Self::Pair(pair) => Some(pair.key.to_string()),
            Self::Word(word) => Some(word.to_string()),
            Self::Raw(_) => None,
        }
    }
}
