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

use banter::{App, Command, Flow, ensure};

struct Game {
    secret: u32,
    tries: u32,
}

fn main() -> ExitCode {
    let app = App::new(|_| {
        Ok(Game {
            secret: (RandomState::new().hash_one(0) % 100) as u32,
            tries: 0,
        })
    })
    .prompt("guess> ")
    .greet(|s, _| s.say("I picked a number below 100. type 'help' for commands."))
    .command(
        Command::new("guess", |s, game: &mut Game, args| {
            let n: u32 = args.get(0)?;
            ensure!(n < 100, "below 100, I said");
            game.tries += 1;
            if n == game.secret {
                s.ok(format!("got it in {}", game.tries))?;
                return Ok(Flow::Close);
            }
            s.say(if n < game.secret { "higher" } else { "lower" })?;
            Ok(Flow::Continue)
        })
        .alias("g")
        .usage("<n>")
        .help("take a guess"),
    );

    banter::run(app)
}
