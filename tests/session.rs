//! End-to-end behaviour of the loop, through the same path a real peer takes.

use std::sync::Arc;

use banter::{
    App, Command, ConnInfo, Flow, Hex, ensure,
    pow::Pow,
    style::{Ansi, Palette},
    testing::{Client, transcript},
};

#[derive(Default)]
struct Counter {
    n: u64,
}

fn counter() -> App<Counter> {
    App::new(|_| Ok(Counter::default()))
        .prompt("c> ")
        .greet(|s, _| s.say("hello"))
        .command(
            Command::new("add", |s, c: &mut Counter, args| {
                let k: u64 = args.or(0, 1)?;
                ensure!(k <= 10, "at most 10 at a time");
                c.n += k;
                s.kv("n", c.n.to_string())?;
                Ok(Flow::Continue)
            })
            .alias("a")
            .usage("[k]")
            .help("add k"),
        )
        .command(
            Command::new("echo", |s, _, args| {
                let Hex(bytes) = args.get(1)?;
                s.kv(args.get::<String>(0)?, banter::hex::encode(bytes))?;
                Ok(Flow::Continue)
            })
            .usage("<label> <hex>")
            .help("echo a blob"),
        )
        .cmd("done", "finish", |s, c, _| {
            s.ok(format!("final {}", c.n))?;
            Ok(Flow::Close)
        })
}

#[test]
fn a_script_runs_to_the_end() {
    let out = transcript(&counter(), "add\na 5\n# comment\n\nadd 20\ndone\nadd\n").unwrap();
    assert!(out.starts_with("hello\nc> "));
    assert!(out.contains("  n  1\n"));
    assert!(out.contains("  n  6\n"));
    assert!(out.contains("error: at most 10 at a time"));
    assert!(out.ends_with("final 6\n"), "{out}");
}

#[test]
fn arity_comes_from_usage() {
    let out = transcript(&counter(), "echo x\n").unwrap();
    assert!(out.contains("error: usage: echo <label> <hex>"));
}

#[test]
fn bad_arguments_are_positioned() {
    let out = transcript(&counter(), "echo x zz\n").unwrap();
    assert!(
        out.contains("error: argument 1 (hex): invalid hex digit 'z'"),
        "{out}"
    );
}

#[test]
fn typos_get_a_suggestion() {
    let out = transcript(&counter(), "dne\nxyzzy\n").unwrap();
    assert!(out.contains("unknown command 'dne'\n  did you mean 'done'?"));
    assert!(out.contains("unknown command 'xyzzy'\n  try 'help'"));
}

#[test]
fn help_lists_commands_and_builtins() {
    let out = transcript(&counter(), "help\n").unwrap();
    for line in ["add [k]", "echo <label> <hex>", "done", "help", "exit"] {
        assert!(out.contains(line), "missing {line} in {out}");
    }
}

#[test]
fn builtins_can_be_dropped() {
    let out = transcript(&counter().without_builtins(), "help\n").unwrap();
    assert!(out.contains("unknown command 'help'"));
}

#[test]
fn overlong_lines_end_the_session() {
    let app = counter().max_line(8);
    let out = transcript(&app, "add 1234567890\nadd\n").unwrap();
    assert!(out.contains("error: line exceeds 8 bytes"));
    assert!(!out.contains("  n  1"));
}

#[test]
fn an_interactive_client_reads_between_commands() {
    let mut client = Client::connect(counter()).unwrap();
    assert_eq!(client.greeting(), "hello\n");
    assert_eq!(client.cmd("add 3").unwrap(), "  n  3\n");
    assert_eq!(client.cmd("add 4").unwrap(), "  n  7\n");
    assert_eq!(client.finish("done").unwrap(), "final 7\n");
}

#[test]
fn sessions_are_independent() {
    let app = Arc::new(counter());
    let mut a = Client::connect(Arc::clone(&app)).unwrap();
    let mut b = Client::connect(app).unwrap();
    a.cmd("add 5").unwrap();
    assert_eq!(b.cmd("add").unwrap(), "  n  1\n");
}

#[test]
fn gates_run_before_init() {
    let app = counter().gate(|s: &mut banter::Session| {
        let word = s.ask("word? ")?;
        if word == "please" {
            Ok(())
        } else {
            Err(banter::Error::Denied("no".into()))
        }
    });
    assert!(transcript(&app, "please\nadd\n").unwrap().contains("hello"));
    let refused = transcript(&app, "nope\nadd\n").unwrap();
    assert_eq!(refused, "word? error: no\n");
}

#[test]
fn proof_of_work_admits_a_correct_answer() {
    let mut client = Client::open(counter().gate(Pow::new(8)), ConnInfo::default());
    let challenge = client.until("pow> ").unwrap();
    let prefix = challenge.split('"').nth(1).unwrap();
    client.send(&Pow::solve(prefix, 8)).unwrap();
    assert_eq!(client.prompt().unwrap(), "hello\n");
    assert_eq!(client.cmd("add").unwrap(), "  n  1\n");
}

#[test]
fn proof_of_work_turns_away_a_wrong_answer() {
    let out = transcript(&counter().gate(Pow::new(32)), "nope\nadd\n").unwrap();
    assert!(out.ends_with("error: proof of work rejected\n"), "{out}");
    assert!(!out.contains("hello"));
}

#[test]
fn ansi_style_paints_prompt_and_errors() {
    let app = counter().style(Ansi(Palette::DEFAULT));
    let out = transcript(&app, "nope\n").unwrap();
    assert!(out.contains("\x1b[1;36mc> \x1b[0m"));
    assert!(out.contains("\x1b[1;31merror: unknown command 'nope'\x1b[0m"));
}
