# banter

Tiny framework for line-based interactive services. Describe per-session state and
its commands; banter owns the prompt loop, typed arguments, help, typo suggestions,
error reporting and the transport. The same `App` serves stdio, TCP, a Unix socket,
or an in-memory test client.

```rust
use banter::{App, Flow, Result, Session, ensure};

struct Game {
    secret: u32,
}

#[banter::commands]
impl Game {
    /// take a guess
    #[command(alias = "g")]
    fn guess(&mut self, s: &mut Session, n: u32) -> Result<Flow> {
        ensure!(n < 100, "below 100, I said");
        if n == self.secret {
            s.ok("got it")?;
            return Ok(Flow::Close);
        }
        s.say(if n < self.secret { "higher" } else { "lower" })?;
        Ok(Flow::Continue)
    }
}

fn main() -> std::process::ExitCode {
    let app = App::new(|_| Ok(Game { secret: 42 }))
        .prompt("guess> ")
        .greet(|s, _| s.say("I picked a number below 100."))
        .commands();
    banter::run(app)
}
```

```
$ ./guess --listen 1337 &
$ nc localhost 1337
I picked a number below 100.
guess> gues 50
error: unknown command 'gues'
  did you mean 'guess'?
guess> guess
error: usage: guess <n>
guess> guess 50
lower
```

## Commands

`#[banter::commands]` on an `impl` block turns every method with a receiver into a
command:

| In the method | Becomes |
|---|---|
| `&self` / `&mut self` | the session state |
| `s: &mut Session` | the session, for output |
| `n: T` | required argument `<n>`, parsed with `FromArg` |
| `n: Option<T>` | optional argument `[n]` |
| `n: Vec<T>` | the rest of the line `[n...]` |
| first doc paragraph | help text |
| `#[command(name, alias, group, tag = Type, hidden, skip)]` | the rest of the spec |

Return `banter::Result<()>` to carry on or `banter::Result<Flow>` to decide.
`#[derive(FromArg)]` makes a unit enum an argument type, matched by kebab-case name.

Without macros, the same is `app.command(Spec::new("guess").arg("n"), |ctx| ...)`.

## Extension points

Every policy is a trait with a default, and closures implement all of them but
`net::Listener`.

| Trait | Default | Decides |
|---|---|---|
| `Lexer` | `Words` (also `Shell` for quotes) | how a line splits into tokens |
| `Suggest` | `Distance` | typo suggestions |
| `Reporter` | `Report` | how errors look |
| `Style` | `Plain` (also `Ansi` with a `Palette`) | how each `Role` looks |
| `Gate` | none (`Pow` for proof of work) | who gets a session |
| `net::Listener` | `Tcp`, `Unix` | where connections come from |
| `net::Observer` | `Log` (also `Silent`) | server events |
| `net::Spawn` | `Threads` | where sessions run |

Hooks on `App`: `greet`, `before` and `after` every command (with `ctx.tagged::<T>()`
for tag checks), `fallback` for unknown input, `close`. The per-session `Store` lets
gates, hooks and `init` share typed data. Built-in `help` and `exit` are ordinary
commands: rename, replace or drop them.

## Features

| Feature | Adds |
|---|---|
| `net` (default) | `net::Server` over TCP or Unix sockets; `--listen` in `banter::run` |
| `macros` (default) | `#[commands]`, `#[derive(FromArg)]` |
| `style` | `Ansi`, `Palette`, `Paint`, `Color` |
| `pow` | `pow::Pow` gate and `pow::Challenge` |
| `testing` | `testing::Script`, `testing::Client` |

## Command line

`banter::run(app)` reads `--listen ADDR` (`1337`, `:1337`, `host:port`,
`unix:PATH`), `--limit N` and `--idle SECS`. `cli::Options::env("PREFIX")` also reads
`PREFIX_LISTEN` and friends.

## Testing

```rust
use banter::testing::{Client, Script};

let out = Script::new("guess 50\nguess 42\n").run(&app)?;
assert!(out.contains("got it"));

let mut client = Client::connect(app)?;
let reply = client.cmd("guess 50")?;
```

## License

MIT or Apache-2.0, at your option.
