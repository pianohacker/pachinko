use clap::{AppSettings, Clap};
use rustyline::Editor;
use shell_words;

use crate::{AHResult, CommonOpts, SubCommand};

#[derive(Clap)]
#[clap(setting = AppSettings::NoBinaryName)]
struct ConsoleOpts {
    #[clap(subcommand)]
    subcmd: ConsoleSubCommand,
}

#[derive(Clap)]
enum ConsoleSubCommand {
    #[clap(flatten)]
    Base(SubCommand),

    #[clap(about = "Quit the console")]
    Quit,
}

pub(crate) fn run_console(opts: CommonOpts) -> AHResult<()> {
    // Make sure the store can be opened before we try to run any commands.
    opts.open_store().unwrap();

    let mut rl = Editor::<()>::new();

    while let Ok(line) = rl.readline("pachinko> ") {
        let continue_console = || -> AHResult<bool> {
            let words = shell_words::split(&line)?;

            if words.len() == 0 {
                return Ok(true);
            }

            if words[0] == "help" {
                <ConsoleOpts as clap::IntoApp>::into_app()
                    .help_template("Available commands:\n{subcommands}")
                    .print_help()?;

                return Ok(true);
            }

            let console_opts = ConsoleOpts::try_parse_from(words)?;

            match console_opts.subcmd {
                ConsoleSubCommand::Quit => Ok(false),
                ConsoleSubCommand::Base(SubCommand::Console(_)) => Ok(true),
                ConsoleSubCommand::Base(sc) => sc.invoke().map(|_| true),
            }
        }()
        .unwrap_or_else(|e| {
            println!("Error: {}", e);

            true
        });

        if !continue_console {
            break;
        }
    }

    Ok(())
}
