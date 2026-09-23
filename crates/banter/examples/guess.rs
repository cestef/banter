//! The smallest useful service: guess a number.
//!
//! ```text
//! cargo run --example guess                    one session on stdio
//! cargo run --example guess -- --listen 1337   serve TCP
//! ```

use std::{
    hash::{BuildHasher, RandomState},
    process::ExitCode,
};

use banter::{App, Flow, Result, Session, ensure};

struct Game {
    secret: u32,
    tries: u32,
}

#[banter::commands]
impl Game {
    const RANGE: u32 = 100;

    fn new() -> Self {
        Self {
            secret: (RandomState::new().hash_one(0) % u64::from(Self::RANGE)) as u32,
            tries: 0,
        }
    }

    /// take a guess
    #[command(alias = "g")]
    fn guess(&mut self, s: &mut Session, n: u32) -> Result<Flow> {
        ensure!(n < Self::RANGE, "below {}, I said", Self::RANGE);
        self.tries += 1;
        if n == self.secret {
            s.ok(format!("got it in {}", self.tries))?;
            return Ok(Flow::Close);
        }
        s.say(if n < self.secret { "higher" } else { "lower" })?;
        Ok(Flow::Continue)
    }
}

fn main() -> ExitCode {
    let app = App::new(|_| Ok(Game::new()))
        .prompt("guess> ")
        .greet(|s, _| s.say("I picked a number below 100. type 'help' for commands."))
        .commands();
    banter::run(app)
}
