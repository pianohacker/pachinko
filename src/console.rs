use clap::{AppSettings, Clap};
use qualia::{Object, Store, Q};
use regex::Regex;
use rustyline::Editor;
use shell_words;
use std::convert::TryFrom;
use std::iter;

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

/// Holds a single word from the input.
#[derive(Debug, Eq, PartialEq)]
struct InputWord {
    /// The starting position of the word. This will come before any delimiters.
    pos: usize,

    /// The contents of the word (backslashes removed).
    word: String,
}

fn unquote(input: &str) -> String {
    let mut chars = input.chars();
    let mut result = String::new();

    macro_rules! read_or {
        ($iter:expr$(,)?, $or:tt) => {
            match $iter.next() {
                Some(c) => c,
                None => {
                    $or;
                }
            }
        };
    }

    loop {
        let next = read_or!(chars, break);

        match next {
            '\\' => {
                let next = read_or!(chars, break);
                result.push(next);
            }
            '"' => {}
            _ => result.push(next),
        };
    }

    result
}

fn words_up_to_cursor_pos(input: impl AsRef<str>, pos: usize) -> Vec<InputWord> {
    let input = input.as_ref();

    let result: Vec<_> = Regex::new(r#"((?:\\"|[^" ]|(")(?:\\"|[^"])+(?:"|$))+)(?:\s+|$)"#)
        .unwrap()
        .captures_iter(input)
        .take_while(|c| c.get(1).unwrap().start() <= pos)
        .filter_map(|c| {
            let mut range = c.get(1).unwrap().range();
            let word_pos = range.start;
            range.end = range.end.min(pos);
            let word = unquote(&input[range]);

            if word != "" {
                Some(InputWord {
                    pos: word_pos,
                    word,
                })
            } else {
                None
            }
        })
        .collect();

    if result.len() == 0 {
        vec![InputWord {
            pos,
            word: "".to_string(),
        }]
    } else {
        result
    }
}

struct ConsoleHelper<'store> {
    store: &'store Store,
}

impl rustyline::Helper for ConsoleHelper<'_> {}

impl rustyline::completion::Completer for ConsoleHelper<'_> {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let _ = (line, pos, ctx);

        let words = words_up_to_cursor_pos(line, pos);

        if words.len() == 1 {
            return Ok((
                words[0].pos,
                vec![
                    "add".to_string(),
                    "item".to_string(),
                    "quickadd".to_string(),
                ],
            ));
        }

        Ok((0, Vec::with_capacity(0)))
    }
}

impl rustyline::highlight::Highlighter for ConsoleHelper<'_> {}

impl rustyline::hint::Hinter for ConsoleHelper<'_> {
    type Hint = String;
}

impl rustyline::validate::Validator for ConsoleHelper<'_> {}

pub(crate) fn run_console(opts: CommonOpts) -> AHResult<()> {
    let store = opts.open_store().unwrap();

    let mut rl = Editor::<ConsoleHelper>::new();
    rl.set_helper(Some(ConsoleHelper { store: &store }));

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

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! word {
        ($pos:expr, $word:expr$(,)?) => {
            InputWord {
                pos: $pos,
                word: $word.to_string(),
            }
        };
    }

    #[test]
    fn words_up_to_cursor_pos_works_on_trival_input() {
        assert_eq!(
            words_up_to_cursor_pos("", 0),
            vec![word!(0, "".to_string())]
        );
        assert_eq!(
            words_up_to_cursor_pos("oneword", 0),
            vec![word!(0, "".to_string())]
        );
        assert_eq!(
            words_up_to_cursor_pos("oneword", 2),
            vec![word!(0, "on".to_string())]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_on_unquoted_words() {
        assert_eq!(
            words_up_to_cursor_pos("two words", 2),
            vec![word!(0, "tw".to_string())]
        );

        assert_eq!(
            words_up_to_cursor_pos("two words", 6),
            vec![word!(0, "two".to_string()), word!(4, "wo".to_string())]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_extra_whitespace() {
        assert_eq!(
            words_up_to_cursor_pos("   two   words ", 10),
            vec![word!(3, "two".to_string()), word!(9, "w".to_string())]
        );

        assert_eq!(
            words_up_to_cursor_pos("   ", 2),
            vec![word!(2, "".to_string())]
        );
        assert_eq!(
            words_up_to_cursor_pos("   ", 3),
            vec![word!(3, "".to_string())]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_quoted_words() {
        assert_eq!(
            words_up_to_cursor_pos(r#"   two   "quoted words" "#, 18),
            vec![
                word!(3, "two".to_string()),
                word!(9, "quoted w".to_string())
            ]
        );

        assert_eq!(
            words_up_to_cursor_pos(r#"   two   "quoted words" "#, 23),
            vec![
                word!(3, "two".to_string()),
                word!(9, "quoted words".to_string())
            ]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_quoted_words_with_backslashes() {
        assert_eq!(
            words_up_to_cursor_pos(r#"three "quo ted" "wo\"rds""#, 22),
            vec![
                word!(0, "three".to_string()),
                word!(6, "quo ted".to_string()),
                word!(16, "wo\"r".to_string())
            ]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_incomplete_quoted_words() {
        assert_eq!(
            words_up_to_cursor_pos(r#"   two   "quoted "#, 17),
            vec![word!(3, "two".to_string()), word!(9, "quoted ".to_string())]
        );
    }
}
