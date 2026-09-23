//! Lifecycle hooks and the per-session store.

mod common;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use banter::{App, Flow, Session, ensure, testing::Script};
use common::{Counter, Paid, counter};

struct Budget(u32);

impl Default for Budget {
    fn default() -> Self {
        Self(2)
    }
}

#[test]
fn before_hooks_can_ration_tagged_commands() {
    let app = counter().before(|ctx| {
        if ctx.tagged::<Paid>() {
            let budget = ctx.store_mut().get_or_default::<Budget>();
            ensure!(budget.0 > 0, "out of queries");
            budget.0 -= 1;
        }
        Ok(())
    });
    let out = Script::new("add\nadd\nadd\necho x ff\n").run(&app).unwrap();
    assert_eq!(out.matches("  n  ").count(), 2);
    assert!(out.contains("error: out of queries"));
    assert!(out.contains("  x  ff"));
}

#[test]
fn after_hooks_see_the_outcome() {
    let failures = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&failures);
    let app = counter().after(move |_, outcome| {
        if outcome.is_err() {
            seen.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    });
    Script::new("add 50\nadd\necho\n").run(&app).unwrap();
    assert_eq!(failures.load(Ordering::SeqCst), 2);
}

#[test]
fn a_fallback_takes_unknown_lines() {
    let app = counter().fallback(|ctx| {
        let words = ctx.args.rest(0);
        let line = [ctx.name, words.as_str()].join(" ");
        ctx.say(line.trim())?;
        Ok(Flow::Continue)
    });
    let out = Script::new("hi there\nadd\n").run(&app).unwrap();
    assert!(out.contains("c> hi there\n"));
    assert!(out.contains("  n  1"));
}

#[test]
fn close_hooks_run_however_the_session_ends() {
    let closed = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&closed);
    let app = counter().close(move |s, c| {
        seen.fetch_add(1, Ordering::SeqCst);
        s.note(format!("closing at {}", c.n))
    });
    assert!(
        Script::new("add\ndone\n")
            .run(&app)
            .unwrap()
            .ends_with("closing at 1\n")
    );
    Script::new("add\n").run(&app).unwrap();
    assert_eq!(closed.load(Ordering::SeqCst), 2);
}

#[test]
fn init_reads_what_gates_stored() {
    struct Start(u64);
    let app = App::new(|s| {
        let n = s.store().get::<Start>().map_or(0, |start| start.0);
        Ok(Counter { n })
    })
    .gate(|s: &mut Session| {
        s.store_mut().insert(Start(40));
        Ok(())
    })
    .commands();
    assert!(
        Script::new("add 2\n")
            .run(&app)
            .unwrap()
            .contains("  n  42")
    );
}
