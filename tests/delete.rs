#[macro_use]
mod common;
use common::*;

#[test]
fn items_should_be_deletable() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item");
    ctx.assert_pch(&["add", "Test/1", "Don't delete me"])
        .only_stdout_contains("Test/1: Don't delete me");
    ctx.assert_pch(&["delete", "Test"])
        .only_stdout_contains("Deleted Test/4: Test item");
    ctx.assert_pch(&["items"])
        .only_stdout_matches(r"^Test/1: Don't delete me \(S\)\n$");
    ctx.assert_pch(&["delete", "delete"])
        .only_stdout_contains("Deleted Test/1: Don't delete me");
    ctx.assert_pch(&["items"]).is_silent();
}

#[test]
fn deleting_should_be_undoable() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item");
    ctx.assert_pch(&["delete", "Test"])
        .only_stdout_contains("Deleted Test/4: Test item");
    ctx.assert_pch(&["items"]).is_silent();
    ctx.assert_pch(&["undo"])
        .only_stdout_contains("Undid: delete items matching Test");
    ctx.assert_pch(&["items"])
        .only_stdout_contains("Test/4: Test item");
}

#[test]
fn deleting_multiple_items_without_confirmation_should_fail() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item");
    ctx.assert_pch(&["add", "Test/1", "Also test item"])
        .only_stdout_contains("Test/1: Also test item");
    ctx.assert_pch_fails(&["delete", "Test"])
        .only_stderr_matches(r"Also test item.*\n.*Test item");
}

#[test]
fn deleting_multiple_items_with_confirmation_should_be_possible() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item");
    ctx.assert_pch(&["add", "Test/1", "Don't delete me"])
        .only_stdout_contains("Test/1: Don't delete me");
    ctx.assert_pch(&["add", "Test/1", "Also test item"])
        .only_stdout_contains("Test/1: Also test item");
    ctx.assert_pch(&["delete", "--all", "Test"])
        .only_stdout_contains(
            "Deleted Test/1: Also test item (S)
Deleted Test/4: Test item (S)",
        );
    ctx.assert_pch(&["items"])
        .only_stdout_matches("Test/1: Don't delete me");
}
