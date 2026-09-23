//! What the macros generate.

mod common;

use banter::{Commands, FromArg, Kind, Result, Session, testing::Script};
use common::{Counter, Paid};

#[test]
fn methods_become_specs() {
    let commands = Counter::commands();
    let add = commands[0].spec();
    assert_eq!(add.name, "add");
    assert_eq!(add.aliases, ["a"]);
    assert_eq!(add.help, "add k");
    assert_eq!(add.params[0].kind, Kind::Optional);
    assert!(add.tagged::<Paid>());
    assert_eq!(
        commands[1].spec().signature().to_string(),
        "echo <label> <blob>"
    );
    assert_eq!(commands[1].spec().group, Some("misc"));
}

#[derive(Debug, PartialEq, FromArg)]
enum Mode {
    Fast,
    SlowAndSteady,
    #[arg(name = "x")]
    Other,
}

#[test]
fn enums_parse_by_kebab_case() {
    assert_eq!(Mode::from_arg("fast"), Ok(Mode::Fast));
    assert_eq!(Mode::from_arg("SLOW-AND-STEADY"), Ok(Mode::SlowAndSteady));
    assert_eq!(Mode::from_arg("x"), Ok(Mode::Other));
    assert_eq!(
        Mode::from_arg("zip"),
        Err("expected one of fast, slow-and-steady, x".into())
    );
    assert_eq!(Mode::KIND, "mode");
}

#[derive(Default)]
struct Shelf {
    mode: Option<Mode>,
    words: Vec<String>,
}

#[banter::commands]
impl Shelf {
    const LIMIT: usize = 3;

    /// set the mode
    ///
    /// Only the first paragraph becomes help.
    fn mode(&mut self, mode: Mode) -> Result<()> {
        self.mode = Some(mode);
        Ok(())
    }

    /// store words
    #[command(name = "put", hidden)]
    fn store(&mut self, words: Vec<String>) -> Result<()> {
        self.words.extend(words.into_iter().take(Self::LIMIT));
        Ok(())
    }

    fn show(&self, s: &mut Session) -> Result<()> {
        s.kv("mode", format!("{:?}", self.mode))?;
        s.kv("words", self.words.join(","))
    }

    #[command(skip)]
    fn helper(&self) -> usize {
        self.words.len()
    }

    fn associated() -> u8 {
        1
    }
}

#[test]
fn a_whole_app_from_one_impl_block() {
    let specs = Shelf::commands();
    assert_eq!(specs.len(), 3);
    assert_eq!(specs[0].spec().help, "set the mode");
    assert!(specs[1].spec().hidden);

    let app = banter::App::new(|_| Ok(Shelf::default())).commands();
    let out = Script::new("mode slow-and-steady\nput a b c d\nshow\nmode nope\n")
        .run(&app)
        .unwrap();
    assert!(out.contains("  mode  Some(SlowAndSteady)"), "{out}");
    assert!(out.contains("  words  a,b,c"));
    assert!(out.contains("error: mode (mode): expected one of"));
    assert_eq!(
        Shelf::default().helper() + usize::from(Shelf::associated()),
        1
    );
}
