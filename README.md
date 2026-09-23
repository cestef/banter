# banter

Tiny framework for line-based interactive services. Describe per-session state and a
command table; banter owns the prompt loop, typed arguments, help, typo suggestions,
error reporting and the transport. The same `App` serves stdio, TCP, or an
in-memory test client.

The core has no dependencies.

```rust
use banter::{App, Command, Flow, ensure};

fn main() -> std::process::ExitCode {
    let app = App::new(|_| Ok(42u32))
        .prompt("guess> ")
        .greet(|s, _| s.say("I picked a number below 100."))
        .command(
            Command::new("guess", |s, secret: &mut u32, args| {
                let n: u32 = args.get(0)?;
                ensure!(n < 100, "below 100, I said");
                if n == *secret {
                    s.ok("got it")?;
                    return Ok(Flow::Close);
                }
                s.say(if n < *secret { "higher" } else { "lower" })?;
                Ok(Flow::Continue)
            })
            .usage("<n>")
            .help("take a guess"),
        );

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
higher
```

## Pieces

| | |
|---|---|
| `App<S>` | state factory, greeting, commands, gates, style, prompt |
| `Command<S>` | name, aliases, `usage` (required `<args>` set the arity), help, handler |
| `Args`, `FromArg` | typed, positioned argument parsing; implement `FromArg` for your types |
| `Session` | what handlers write through: `say`, `kv`, `block`, `table`, `ask`, `line` |
| `Gate` | runs before a session starts; any `Fn(&mut Session) -> Result<()>` is one |
| `Style`, `Role` | output is tagged by role, a style decides how roles look |
| `Error` | `reject!`/`ensure!` refuse a request and keep the session alive |

Handlers return `Flow::Continue` or `Flow::Close`. Errors the peer caused are
reported as `error: ...` and the loop goes on; I/O errors, timeouts and overlong lines
end the session.

## Features

| Feature | Adds |
|---|---|
| `tcp` (default) | `tcp::Server`: thread per connection, connection cap, idle timeout; `--listen` in `banter::run` |
| `style` | `style::Ansi` with palettes |
| `pow` | `pow::Pow`, a SHA-256 proof-of-work gate, and `Pow::solve` for clients |
| `testing` | `testing::transcript` for scripted sessions, `testing::Client` for interactive ones |

## Command line

`banter::run(app)` understands:

```
--listen ADDR     serve TCP instead of stdio (1337, :1337, host:port)
--max-conns N     concurrent sessions (default 64)
--idle SECS       drop silent peers (default 120, 0 disables)
```

For anything else, build a `tcp::Server` or call `App::serve(Conn)` yourself.

## Testing

```rust
use banter::testing::{Client, transcript};

let out = transcript(&app, "guess 50\nguess 42\n")?;
assert!(out.contains("got it"));

let mut client = Client::connect(app)?;
let reply = client.cmd("guess 50")?;   // everything up to the next prompt
```

## License

MIT or Apache-2.0, at your option.
