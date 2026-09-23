//! Gates, styles, typed arguments and runtime style switching together.
//!
//! ```text
//! cargo run --example vault --features style,pow -- --listen 1337
//! ```

use std::{process::ExitCode, sync::Arc};

use banter::{
    App, Block, Cell, Command, Flow, Hex, Plain, Role, Session, ensure,
    pow::Pow,
    style::{Ansi, Palette},
};

#[derive(Default)]
struct Vault {
    items: Vec<Vec<u8>>,
}

/// A gate is any closure over the session.
fn motd(session: &mut Session) -> banter::Result<()> {
    session.styled(Role::Title, "vault")?;
    session.note("store blobs, read them back.")
}

fn main() -> ExitCode {
    let app = App::new(|_| Ok(Vault::default()))
        .prompt("vault> ")
        .style(Ansi(Palette::DEFAULT))
        .gate(Pow::new(8))
        .gate(motd)
        .command(
            Command::new("put", |s, v: &mut Vault, args| {
                let Hex(blob) = args.get(0)?;
                ensure!(v.items.len() < 16, "the vault is full");
                v.items.push(blob);
                s.kv(
                    "slot",
                    Cell::new(Role::Value, (v.items.len() - 1).to_string()),
                )?;
                Ok(Flow::Continue)
            })
            .usage("<hex>")
            .help("store a blob"),
        )
        .command(
            Command::new("get", |s, v: &mut Vault, args| {
                let slot: usize = args.get(0)?;
                let Some(blob) = v.items.get(slot) else {
                    banter::reject!("slot {slot} is empty");
                };
                s.block(
                    &Block::new()
                        .entry("slot", slot.to_string())
                        .entry("hex", banter::hex::encode(blob))
                        .entry("len", blob.len().to_string()),
                )?;
                Ok(Flow::Continue)
            })
            .usage("<slot>")
            .help("read a blob back"),
        )
        .cmd("plain", "turn colour off", |s, _, _| {
            s.set_style(Arc::new(Plain));
            Ok(Flow::Continue)
        });

    banter::run(app)
}
