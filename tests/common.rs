use predicates::prelude::*;
pub use tempfile::{Builder, TempDir};

pub struct TestContext {
    pub temp_dir: TempDir,
}

#[allow(dead_code)]
impl TestContext {
    pub fn pch_cmd(&self, arguments: &[&str]) -> std::process::Command {
        let mut cmd = std::process::Command::new(assert_cmd::cargo::cargo_bin("pachinko"));

        cmd.args(arguments)
            .current_dir(self.temp_dir.path())
            .env("HOME", self.temp_dir.path())
            .env("PACHINKO_STORE_PATH", self.store_path());

        cmd
    }

    pub fn pch_assert_cmd(&self, arguments: &[&str]) -> assert_cmd::Command {
        assert_cmd::Command::from(self.pch_cmd(arguments))
    }

    pub fn store_path(&self) -> String {
        self.temp_dir
            .path()
            .join("pachinko-test-store.qualia")
            .to_string_lossy()
            .into_owned()
    }

    pub fn assert_pch(&self, arguments: &[&str]) -> assert_cmd::assert::Assert {
        self.pch_assert_cmd(arguments).assert().success()
    }

    pub fn assert_pch_fails(&self, arguments: &[&str]) -> assert_cmd::assert::Assert {
        self.pch_assert_cmd(arguments).assert().failure()
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
        let temp_dir = Builder::new().prefix("pachinko-cli").tempdir().unwrap();

        let $ctx = TestContext { temp_dir };
    };
}
