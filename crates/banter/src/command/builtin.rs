//! Ready-made commands. [`App::new`](crate::App::new) installs [`Builtin::help`] and
//! [`Builtin::exit`]; replace, rename or drop them like any other command.

use super::{Command, Flow, Spec};
use crate::output::{Cell, Role, Table};

/// Constructors for the default commands.
pub struct Builtin;

impl Builtin {
    /// Lists every visible command, grouped, with its signature and help.
    pub fn help<S>() -> Command<S> {
        let spec = Spec::new("help").alias("?").help("show this list");
        Command::new(spec, |ctx| {
            for (group, commands) in ctx.app.registry().sections() {
                if let Some(group) = group {
                    ctx.styled(Role::Title, group)?;
                }
                let mut table = Table::new();
                for command in commands {
                    let spec = command.spec();
                    table.push([
                        Cell::new(Role::Key, spec.signature().to_string()),
                        Cell::new(Role::Dim, spec.help),
                    ]);
                }
                ctx.table(&table)?;
            }
            Ok(Flow::Continue)
        })
    }

    /// Ends the session.
    pub fn exit<S>() -> Command<S> {
        let spec = Spec::new("exit").alias("quit").help("close the session");
        Command::new(spec, |_| Ok(Flow::Close))
    }
}
