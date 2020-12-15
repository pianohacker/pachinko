#[macro_use]
mod common;
use common::*;

use std::path::Path;

fn assert_pch_in_home(ctx: &TestContext, args: &[&str]) -> assert_cmd::assert::Assert {
    let mut cmd = ctx.pch_cmd(args);
    cmd.env_remove("PACHINKO_STORE_PATH");

    assert_cmd::Command::from(cmd).assert().success()
}

#[test]
fn default_store_path_correct() {
    init!(ctx);

    assert_pch_in_home(&ctx, &["add-location", "Test", "16"]).is_silent();
    assert_pch_in_home(&ctx, &["locations"]).only_stdout_contains("Test (16 bins)");

    assert!(!Path::new(&ctx.store_path()).exists());
    assert!(ctx
        .temp_dir
        .path()
        .join(".local")
        .join("share")
        .join("pachinko")
        .join("pachinko.qualia")
        .exists());
}
