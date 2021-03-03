use clap::{AppSettings, Clap};
use qualia::{Store, Q};
use regex::Regex;
use rustyline::Editor;
use shell_words;
use std::borrow::Cow;

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
#[derive(Clone, Debug, Eq, PartialEq)]
struct InputWord {
    /// The starting position of the word. This will come before any delimiters.
    pos: usize,

    /// The contents of the word (backslashes removed).
    word: String,

    /// The delimiters (if any) of the word as originally parsed.
    delimiters: String,
}

fn quote(input: &str, existing_delimiters: String) -> String {
    if existing_delimiters == "\"" {
        "\"".to_string() + &input.replace("\\", "\\\\").replace("\"", "\\\"") + "\""
    } else if input.find(|c| c == ' ' || c == '"' || c == '\\').is_some() {
        input
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace(" ", "\\ ")
    } else {
        input.to_string()
    }
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
    let mut farthest_parsed = 0;

    let mut result: Vec<_> = Regex::new(r#"((?:\\[ "]|[^" ]|(")(?:\\"|[^"])+(?:"|$))+)(?:\s+|$)"#)
        .unwrap()
        .captures_iter(input)
        .take_while(|c| c.get(1).unwrap().start() <= pos)
        .filter_map(|c| {
            let mut range = c.get(1).unwrap().range();
            let word_pos = range.start;
            farthest_parsed = range.end;
            range.end = range.end.min(pos);
            let original = &input[range];
            let word = unquote(original);

            if word != "" || word_pos == pos {
                Some(InputWord {
                    pos: word_pos,
                    word,
                    delimiters: c.get(2).map_or("", |g| g.as_str()).to_string(),
                })
            } else {
                None
            }
        })
        .collect();

    if result.len() == 0 || pos > farthest_parsed {
        result.extend(vec![InputWord {
            pos,
            word: "".to_string(),
            delimiters: "".to_string(),
        }]);
        result
    } else {
        result
    }
}

fn filter_and_format_candidates(candidates: Vec<String>, input: &InputWord) -> Vec<String> {
    let mut result = candidates
        .iter()
        .filter_map(|candidate| {
            if candidate.starts_with(&input.word) {
                Some(quote(candidate, input.delimiters.clone()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    result.sort();

    result
}

struct ConsoleHelper<'store> {
    store: &'store Store,
}

impl<'store> ConsoleHelper<'store> {
    fn positional_completion_candidates(&self, argument_name: impl AsRef<str>) -> Vec<String> {
        match argument_name.as_ref() {
            "name-pattern" => self
                .store
                .query(Q.equal("type", "item"))
                .iter_as::<crate::types::Item>()
                .unwrap()
                .map(|item| item.name)
                .collect(),
            _ => vec![],
        }
    }

    fn completion_candidates(&self, words: &Vec<InputWord>) -> Vec<String> {
        let mut words = words.clone();
        let mut app = &<ConsoleOpts as clap::IntoApp>::into_app();

        while words.len() > 1 {
            if let Some(sc) = app
                .get_subcommands()
                .find(|sc| sc.get_name() == words[0].word)
            {
                app = sc;
                words.remove(0);
            } else {
                break;
            }
        }

        let cur_word = words.len() - 1;
        let positional_args = app.get_positionals().collect::<Vec<_>>();

        let candidates = if cur_word == 0 && app.has_subcommands() {
            app.get_subcommands()
                .map(|sc| sc.get_name().to_string())
                .collect()
        } else if cur_word < positional_args.len() {
            self.positional_completion_candidates(positional_args[cur_word].get_name())
        } else {
            vec![]
        };

        filter_and_format_candidates(candidates, &words[words.len() - 1])
    }
}

impl rustyline::Helper for ConsoleHelper<'_> {}

impl rustyline::completion::Completer for ConsoleHelper<'_> {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let words = words_up_to_cursor_pos(line, pos);
        let candidates = self.completion_candidates(&words);

        Ok((words[words.len() - 1].pos, candidates))
    }
}

impl rustyline::highlight::Highlighter for ConsoleHelper<'_> {
    fn highlight_hint<'a>(&self, hint: &'a str) -> Cow<'a, str> {
        format!("\x1b[38;5;244m{}\x1b[0m", hint).into()
    }
}

impl rustyline::hint::Hinter for ConsoleHelper<'_> {
    type Hint = String;
    fn hint(&self, line: &str, pos: usize, _ctx: &rustyline::Context<'_>) -> Option<Self::Hint> {
        let words = words_up_to_cursor_pos(line, pos);
        let last_word = &words[words.len() - 1];

        // Don't hint at the start of a word.
        if last_word.word == "" {
            return None;
        }

        let candidates = self.completion_candidates(&words);

        if candidates.len() == 1 {
            Some(candidates[0][pos - last_word.pos..].to_string())
        } else {
            None
        }
    }
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

    use qualia::{object, Object};
    use tempfile::{Builder, TempDir};

    macro_rules! word {
        ($pos:expr, $word:expr$(,)?) => {
            InputWord {
                pos: $pos,
                word: $word.to_string(),
                delimiters: "".to_string(),
            }
        };

        ($pos:expr, $word:expr, $delimiters:expr$(,)?) => {
            InputWord {
                pos: $pos,
                word: $word.to_string(),
                delimiters: $delimiters.to_string(),
            }
        };
    }

    #[test]
    fn words_up_to_cursor_pos_works_on_trival_input() {
        assert_eq!(words_up_to_cursor_pos("", 0), vec![word!(0, "")]);
        assert_eq!(words_up_to_cursor_pos("oneword", 0), vec![word!(0, "")]);
        assert_eq!(words_up_to_cursor_pos("oneword", 2), vec![word!(0, "on")]);
    }

    #[test]
    fn words_up_to_cursor_pos_includes_empty_trailing_word() {
        assert_eq!(words_up_to_cursor_pos("two ", 3), vec![word!(0, "two")]);
        assert_eq!(
            words_up_to_cursor_pos("two ", 4),
            vec![word!(0, "two"), word!(4, "")]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_on_unquoted_words() {
        assert_eq!(words_up_to_cursor_pos("two words", 2), vec![word!(0, "tw")]);

        assert_eq!(
            words_up_to_cursor_pos("two words", 6),
            vec![word!(0, "two"), word!(4, "wo")]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_extra_whitespace() {
        assert_eq!(
            words_up_to_cursor_pos("   two   words ", 10),
            vec![word!(3, "two"), word!(9, "w")]
        );

        assert_eq!(words_up_to_cursor_pos("   ", 2), vec![word!(2, "")]);
        assert_eq!(words_up_to_cursor_pos("   ", 3), vec![word!(3, "")]);
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_unquoted_words_with_backslashes() {
        assert_eq!(
            words_up_to_cursor_pos(r#"three esc\ aped words"#, 17),
            vec![word!(0, "three"), word!(6, "esc aped"), word!(16, "w")]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_quoted_words() {
        assert_eq!(
            words_up_to_cursor_pos(r#"   two   "quoted words" "#, 18),
            vec![word!(3, "two"), word!(9, "quoted w", "\"")]
        );

        assert_eq!(
            words_up_to_cursor_pos(r#"   two   "quoted words" "#, 23),
            vec![word!(3, "two"), word!(9, "quoted words", "\"")]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_quoted_words_with_backslashes() {
        assert_eq!(
            words_up_to_cursor_pos(r#"three "quo ted" "wo\"rds""#, 22),
            vec![
                word!(0, "three"),
                word!(6, "quo ted", "\""),
                word!(16, "wo\"r", "\"")
            ]
        );
    }

    #[test]
    fn words_up_to_cursor_pos_works_with_incomplete_quoted_words() {
        assert_eq!(
            words_up_to_cursor_pos(r#"   two   "quoted "#, 17),
            vec![word!(3, "two"), word!(9, "quoted ", "\"")]
        );
    }

    #[test]
    fn filter_candidates_works_with_trivial_input() {
        assert_eq!(
            filter_and_format_candidates(
                vec!["abc".to_string(), "deaf".to_string(), "def".to_string(),],
                &word!(0, ""),
            ),
            vec!["abc".to_string(), "deaf".to_string(), "def".to_string(),],
        );
    }

    #[test]
    fn filter_candidates_works_with_prefixes() {
        assert_eq!(
            filter_and_format_candidates(
                vec!["abc".to_string(), "deaf".to_string(), "def".to_string(),],
                &word!(0, "de"),
            ),
            vec!["deaf".to_string(), "def".to_string(),],
        );
    }

    #[test]
    fn filter_candidates_works_with_words_needing_quotes() {
        assert_eq!(
            filter_and_format_candidates(
                vec![
                    "ab cd".to_string(),
                    "a\"c".to_string(),
                    "ef gh".to_string(),
                    "abcd".to_string(),
                    "de af".to_string(),
                    "ab\\c".to_string(),
                ],
                &word!(0, "a", "\""),
            ),
            vec![
                "\"a\\\"c\"".to_string(),
                "\"ab cd\"".to_string(),
                "\"ab\\\\c\"".to_string(),
                "\"abcd\"".to_string(),
            ],
        );
    }

    #[test]
    fn filter_candidates_works_with_words_needing_backslashes() {
        assert_eq!(
            filter_and_format_candidates(
                vec![
                    "ab cd".to_string(),
                    "a\"c".to_string(),
                    "ef gh".to_string(),
                    "abcd".to_string(),
                    "de af".to_string(),
                    "ab\\c".to_string(),
                ],
                &word!(0, "a", ""),
            ),
            vec![
                "a\\\"c".to_string(),
                "ab\\ cd".to_string(),
                "ab\\\\c".to_string(),
                "abcd".to_string(),
            ],
        );
    }

    fn open_test_store() -> (TempDir, Store) {
        let temp_dir = Builder::new().prefix("pachinko-cli").tempdir().unwrap();
        let store_path = temp_dir.path().clone().join("pachinko-test-store.qualia");

        (temp_dir, Store::open(store_path).unwrap())
    }

    #[test]
    fn completion_candidates_completes_initial_command() {
        let (_temp_dir, store) = open_test_store();
        let helper = &ConsoleHelper { store: &store };

        assert_eq!(
            helper.completion_candidates(&vec![word!(0, "")]),
            vec![
                "add".to_string(),
                "add-location".to_string(),
                "console".to_string(),
                "delete".to_string(),
                "items".to_string(),
                "locations".to_string(),
                "quickadd".to_string(),
                "quit".to_string(),
                "undo".to_string(),
            ],
        );

        assert_eq!(
            helper.completion_candidates(&vec![word!(0, "q")]),
            vec!["quickadd".to_string(), "quit".to_string(),],
        );
    }

    #[test]
    fn completion_candidates_offers_no_completions_for_new_input() {
        let (_temp_dir, store) = open_test_store();
        let helper = &ConsoleHelper { store: &store };

        assert_eq!(
            helper.completion_candidates(&vec![word!(0, "add-location"), word!(13, "")]),
            Vec::<String>::new(),
        );
    }

    #[test]
    fn completion_candidates_completes_item_names() {
        let (_temp_dir, mut store) = open_test_store();

        let checkpoint = store.checkpoint().unwrap();
        checkpoint
            .add(object!(
                "type" => "item",
                "name" => "abc",
                "location_id" => 0,
                "bin_no" => 0,
                "size" => "S",
            ))
            .unwrap();
        checkpoint
            .add(object!(
                "type" => "item",
                "name" => "aaa",
                "location_id" => 0,
                "bin_no" => 0,
                "size" => "S",
            ))
            .unwrap();
        checkpoint
            .add(object!(
                "type" => "item",
                "name" => "def",
                "location_id" => 0,
                "bin_no" => 0,
                "size" => "S",
            ))
            .unwrap();
        checkpoint.commit("").unwrap();

        let helper = &ConsoleHelper { store: &store };

        assert_eq!(
            helper.completion_candidates(&vec![word!(0, "delete"), word!(7, "a")]),
            vec!["aaa".to_string(), "abc".to_string()],
        );
    }

    fn get_hint(input: impl AsRef<str>, pos: usize) -> Option<String> {
        let (_temp_dir, mut store) = open_test_store();

        let checkpoint = store.checkpoint().unwrap();
        checkpoint
            .add(object!(
                "type" => "item",
                "name" => "space abc",
                "location_id" => 0,
                "bin_no" => 0,
                "size" => "S",
            ))
            .unwrap();
        checkpoint.commit("").unwrap();

        let helper = &ConsoleHelper { store: &store };

        use rustyline::hint::Hinter;

        helper.hint(
            input.as_ref(),
            pos,
            &rustyline::Context::new(&rustyline::history::History::new()),
        )
    }

    #[test]
    fn hinting_does_nothing_at_the_start_of_a_word() {
        assert_eq!(get_hint("", 0,), None);
    }

    #[test]
    fn hinting_does_nothing_when_multiple_options() {
        assert_eq!(get_hint("a", 1,), None);
    }

    #[test]
    fn hinting_works_for_basic_input() {
        assert_eq!(get_hint("add-", 4), Some("location".to_string()));
        assert_eq!(get_hint("quickadd", 8), Some("".to_string()));
    }

    #[test]
    fn hinting_works_for_quoted_words() {
        assert_eq!(get_hint("\"add-", 5), Some("location\"".to_string()));
        assert_eq!(get_hint("delete \"space ", 14), Some("abc\"".to_string()));
        assert_eq!(get_hint("delete space\\ ", 14), Some("abc".to_string()));
    }
}
