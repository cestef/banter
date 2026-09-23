//! Gates, including proof of work, and styling.

mod common;

use banter::{
    Error, Peer, Role, Session,
    output::{Ansi, Paint, Palette},
    pow::{Challenge, Pow},
    testing::{Client, Script},
};
use common::counter;

#[test]
fn a_gate_runs_before_init_and_can_refuse() {
    let app = counter().gate(|s: &mut Session| match s.ask("word? ")?.as_str() {
        "please" => Ok(()),
        _ => Err(Error::denied("no")),
    });
    assert!(
        Script::new("please\nadd\n")
            .run(&app)
            .unwrap()
            .contains("hello")
    );
    assert_eq!(
        Script::new("nope\nadd\n").run(&app).unwrap(),
        "word? error: no\n"
    );
}

#[test]
fn proof_of_work_admits_a_correct_answer() {
    let pow = Pow::new(8).prefix(|_| "fixed".into());
    let mut client = Client::open(counter().gate(pow), Peer::Local);
    client.until("pow> ").unwrap();
    let answer = Challenge {
        prefix: "fixed".into(),
        bits: 8,
    }
    .solve();
    client.send(&answer).unwrap();
    let mut client = client.greet().unwrap();
    assert_eq!(client.greeting(), "hello\n");
    assert_eq!(client.cmd("add").unwrap(), "  n  1\n");
}

#[test]
fn proof_of_work_text_is_configurable() {
    let pow = Pow::new(32)
        .prompt("? ")
        .refusal("nope")
        .prefix(|_| "fixed".into())
        .message(|c: &Challenge| [c.prefix.as_str(), "?"].concat());
    let out = Script::new("x\n").run(&counter().gate(pow)).unwrap();
    assert_eq!(out, "fixed?\n? error: nope\n");
}

#[test]
fn ansi_paints_prompt_errors_and_custom_roles() {
    const MARK: Role = Role::Custom(0);
    let palette = Palette::colour().set(MARK, Paint::new().underline());
    let app = counter()
        .style(Ansi(palette))
        .greet(|s, _| s.styled(MARK, "u"));
    let out = Script::new("nope\n").run(&app).unwrap();
    assert!(out.contains("\x1b[1;36mc> \x1b[0m"));
    assert!(out.contains("\x1b[1;31merror: unknown command 'nope'\x1b[0m"));
    assert!(out.contains("\x1b[4mu\x1b[0m"));
}
