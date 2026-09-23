//! A small app every integration test starts from, declared with the macro.

#![allow(dead_code)]

use banter::{App, Flow, Hex, Result, Session, ensure};

/// Marks commands a budget hook should charge for.
pub struct Paid;

#[derive(Default)]
pub struct Counter {
    pub n: u64,
}

#[banter::commands]
impl Counter {
    /// add k
    #[command(alias = "a", tag = Paid)]
    fn add(&mut self, s: &mut Session, k: Option<u64>) -> Result<()> {
        let k = k.unwrap_or(1);
        ensure!(k <= 10, "at most 10 at a time");
        self.n += k;
        s.kv("n", self.n.to_string())
    }

    /// echo a blob
    #[command(group = "misc")]
    fn echo(&self, s: &mut Session, label: String, blob: Hex) -> Result<()> {
        s.kv(label, banter::hex::encode(blob.0))
    }

    /// finish
    fn done(&self, s: &mut Session) -> Result<Flow> {
        s.ok(format!("final {}", self.n))?;
        Ok(Flow::Close)
    }
}

pub fn counter() -> App<Counter> {
    App::new(|_| Ok(Counter::default()))
        .prompt("c> ")
        .greet(|s, _| s.say("hello"))
        .commands()
}
