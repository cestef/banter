//! Most extension points at once: a proof-of-work gate, a login gate that feeds
//! `init` through the store, a budget enforced by a `before` hook on tagged
//! commands, a dynamic prompt, quoted arguments, an enum argument, custom error
//! text, a custom role, a style that can be switched off, and grouped help.
//!
//! ```text
//! cargo run --example vault --features style,pow -- --listen 1337
//! ```

use std::process::ExitCode;

use banter::{
    App, Block, Error, FromArg, Hex, Plain, Prompt, Report, Result, Role, Session, Shell, ensure,
    output::{Ansi, Color, Paint, Palette},
    pow::Pow,
    reject,
};

/// Who passed the login gate.
struct User(String);

/// Marks commands that cost a query.
struct Paid;

/// Queries left this session.
struct Budget(u32);

impl Default for Budget {
    fn default() -> Self {
        Self(8)
    }
}

/// How `get` prints a blob.
#[derive(FromArg)]
enum Format {
    Hex,
    Text,
}

struct Vault {
    owner: String,
    items: Vec<Vec<u8>>,
}

#[banter::commands]
impl Vault {
    const SLOTS: usize = 16;
    const OWNER: Role = Role::Custom(0);

    fn open(s: &mut Session) -> Self {
        let owner = s.store().get::<User>().map_or("?".into(), |u| u.0.clone());
        Self {
            owner,
            items: Vec::new(),
        }
    }

    /// store a blob
    #[command(group = "storage", tag = Paid)]
    fn put(&mut self, s: &mut Session, blob: Hex) -> Result<()> {
        ensure!(self.items.len() < Self::SLOTS, "the vault is full");
        self.items.push(blob.0);
        s.kv("slot", (self.items.len() - 1).to_string())
    }

    /// read a blob back
    #[command(group = "storage", tag = Paid)]
    fn get(&self, s: &mut Session, slot: usize, format: Option<Format>) -> Result<()> {
        let Some(blob) = self.items.get(slot) else {
            reject!("slot {slot} is empty");
        };
        let shown = match format.unwrap_or(Format::Hex) {
            Format::Hex => banter::hex::encode(blob),
            Format::Text => String::from_utf8_lossy(blob).into_owned(),
        };
        s.block(
            &Block::new()
                .entry("slot", slot.to_string())
                .entry("data", shown)
                .entry("len", blob.len().to_string()),
        )
    }

    /// echo a quoted string
    #[command(group = "misc")]
    fn note(&self, s: &mut Session, text: String) -> Result<()> {
        s.note(text)
    }

    /// turn colour off
    #[command(group = "misc")]
    fn plain(&self, s: &mut Session) -> Result<()> {
        s.restyle(Plain);
        Ok(())
    }
}

fn login(session: &mut Session) -> Result<()> {
    let name = session.ask("name: ")?;
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::denied("a name, please"));
    }
    session.store_mut().insert(User(name.to_string()));
    Ok(())
}

fn main() -> ExitCode {
    let palette = Palette::colour().set(Vault::OWNER, Paint::new().bold().fg(Color::Green));

    let app = App::new(|s| Ok(Vault::open(s)))
        .commands()
        .style(Ansi(palette))
        .lexer(Shell::default())
        .reporter(Report::new().label("! "))
        .prompt(Prompt::dynamic(|_, v: &Vault| {
            format!("vault[{}/{}]> ", v.items.len(), Vault::SLOTS)
        }))
        .gate(Pow::new(8).prompt("work> "))
        .gate(login)
        .greet(|s, v| {
            let owner = s.paint(Vault::OWNER, &v.owner);
            s.say(format!("welcome, {owner}"))
        })
        .before(|ctx| {
            if ctx.tagged::<Paid>() {
                let budget = ctx.store_mut().get_or_default::<Budget>();
                ensure!(budget.0 > 0, "out of queries, reconnect");
                budget.0 -= 1;
            }
            Ok(())
        });

    banter::run(app)
}
