//! The command loop, through the same path a real peer takes.

mod common;

use std::sync::Arc;

use banter::{
    App, Builtin, Distance, Flow, Layout, Newline, Prompt, Report, Shell, Spec,
    testing::{Client, Script},
};
use common::{Counter, counter};

fn run(app: &App<Counter>, input: &str) -> String {
    Script::new(input).run(app).unwrap()
}

#[test]
fn a_script_runs_to_the_end() {
    let out = run(&counter(), "add\na 5\n# comment\n\nadd 20\ndone\nadd\n");
    assert!(out.starts_with("hello\nc> "));
    assert!(out.contains("  n  1\n"));
    assert!(out.contains("  n  6\n"));
    assert!(out.contains("error: at most 10 at a time"));
    assert!(out.ends_with("final 6\n"), "{out}");
}

#[test]
fn arity_comes_from_the_parameters() {
    assert!(run(&counter(), "echo x\n").contains("error: usage: echo <label> <blob>"));
}

#[test]
fn bad_arguments_name_their_parameter() {
    let out = run(&counter(), "echo x zz\n");
    assert!(
        out.contains("error: blob (hex): invalid hex digit 'z'"),
        "{out}"
    );
}

#[test]
fn typos_get_a_suggestion() {
    let out = run(&counter(), "dne\nxyzzy\n");
    assert!(out.contains("unknown command 'dne'\n  did you mean 'done'?"));
    assert!(out.contains("unknown command 'xyzzy'\n  try 'help'"));
}

#[test]
fn suggestions_can_be_turned_off_or_replaced() {
    let off = counter().suggest(None::<Distance>);
    assert!(run(&off, "dne\n").contains("unknown command 'dne'\n  try 'help'"));
    let always = counter().suggest(Some(|_: &str, _: &[&str]| Some("add".to_string())));
    assert!(run(&always, "zzz\n").contains("did you mean 'add'?"));
}

#[test]
fn help_is_grouped_with_builtins_last() {
    let out = run(&counter(), "help\n");
    let add = out.find("add [k]").unwrap();
    let misc = out.find("misc\n").unwrap();
    let echo = out.find("echo <label> <blob>").unwrap();
    let exit = out.find("exit").unwrap();
    assert!(add < misc && misc < echo && echo < exit, "{out}");
}

#[test]
fn builtins_can_be_dropped_replaced_or_renamed() {
    assert!(run(&counter().bare(), "help\n").contains("unknown command 'help'"));
    assert!(run(&counter().remove("exit"), "exit\n").contains("unknown command 'exit'"));

    let bye = counter()
        .remove("exit")
        .builtin(Builtin::exit().respec(|s| s.alias("bye")));
    assert!(run(&bye, "bye\nadd\n").ends_with("c> "));

    let terse = counter().command(Spec::new("help"), |ctx| ctx.say("no help for you"));
    assert!(run(&terse, "help\n").contains("no help for you"));
}

#[test]
fn case_folding_is_opt_in() {
    assert!(run(&counter(), "ADD\n").contains("unknown command"));
    assert!(run(&counter().fold(), "ADD\n").contains("  n  1"));
}

#[test]
fn the_shell_lexer_groups_quotes() {
    let app = counter().lexer(Shell::default());
    assert!(run(&app, "echo \"a b\" ff\n").contains("  a b  ff\n"));
    assert!(run(&app, "echo \"oops\n").contains("error: unterminated \" quote"));
}

#[test]
fn reporting_is_configurable() {
    let app = counter().reporter(Report::new().label("E: ").hints(false));
    assert!(run(&app, "zzz\n").contains("E: unknown command 'zzz'\nc> "));
}

#[test]
fn line_endings_and_layout_are_configurable() {
    let app = counter()
        .newline(Newline::CrLf)
        .layout(Layout { indent: 0, gap: 1 });
    assert!(run(&app, "add\n").contains("hello\r\nc> n 1\r\n"));
}

#[test]
fn overlong_lines_end_the_session() {
    let out = run(&counter().max_line(8), "add 1234567890\nadd\n");
    assert!(out.contains("error: line exceeds 8 bytes"));
    assert!(!out.contains("  n  1"));
}

#[test]
fn prompts_can_follow_state() {
    let app = counter().prompt(Prompt::dynamic(|_, c: &Counter| format!("[{}]> ", c.n)));
    assert!(run(&app, "add 2\n").contains("[0]>   n  2\n[2]> "));
}

#[test]
fn handlers_may_return_unit_or_flow() {
    let app = counter()
        .command(Spec::new("stay"), |_| Ok(()))
        .command(Spec::new("go"), |_| Ok(Flow::Close));
    assert!(run(&app, "stay\ngo\nadd\n").ends_with("c> c> "));
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
