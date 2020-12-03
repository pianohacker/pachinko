use assert_cmd::Command;
use predicates::prelude::*;
pub use tempdir::TempDir;

pub struct TestContext {
    pub temp_dir: TempDir,
}

impl TestContext {
    pub fn pch_cmd(&self, arguments: &[&str]) -> assert_cmd::Command {
        let mut cmd = Command::cargo_bin("pachinko").unwrap();

        cmd.args(arguments).env(
            "PACHINKO_STORE_PATH",
            self.temp_dir.path().join("pachinko-test-store.qualia"),
        );

        cmd
    }

    pub fn assert_pch(&self, arguments: &[&str]) -> assert_cmd::assert::Assert {
        self.pch_cmd(arguments).assert().success()
    }

    #[allow(dead_code)]
    pub fn assert_pch_fails(&self, arguments: &[&str]) -> assert_cmd::assert::Assert {
        self.pch_cmd(arguments).assert().failure()
    }

    pub fn populate(&self) {
        self.assert_pch(&["add-location", "Test", "4"]);
        self.assert_pch(&["add-location", "Tiny", "1"]);
        self.assert_pch(&["add-location", "Huge", "16"]);
    }
}

pub trait CommandAssertHelpers {
    fn is_silent(self) -> Self;
    fn only_stdout_contains(self, s: impl AsRef<str>) -> Self;
    fn only_stdout_matches(self, s: impl AsRef<str>) -> Self;
    fn only_stderr_matches(self, s: impl AsRef<str>) -> Self;
}

impl CommandAssertHelpers for assert_cmd::assert::Assert {
    fn is_silent(self) -> Self {
        self.stdout(predicate::str::is_empty())
            .stderr(predicate::str::is_empty())
    }

    fn only_stdout_contains(self, s: impl AsRef<str>) -> Self {
        self.stderr(predicate::str::is_empty())
            .stdout(predicate::str::contains(s.as_ref()))
    }

    fn only_stdout_matches(self, s: impl AsRef<str>) -> Self {
        self.stderr(predicate::str::is_empty())
            .stdout(predicate::str::is_match(s.as_ref()).unwrap())
    }

    fn only_stderr_matches(self, s: impl AsRef<str>) -> Self {
        self.stdout(predicate::str::is_empty())
            .stderr(predicate::str::is_match(s.as_ref()).unwrap())
    }
}

#[allow(unused_macros)]
macro_rules! init {
    ($ctx:ident) => {
        let temp_dir = TempDir::new("pachinko-cli").unwrap();

        let $ctx = TestContext { temp_dir };
    };
}
