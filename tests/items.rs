#[macro_use]
mod common;
use common::*;

#[test]
fn adding_an_item_to_a_specified_bin() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item");
    ctx.assert_pch(&["items"])
        .only_stdout_contains("Test/4: Test item");
}

#[test]
fn adding_an_item_should_be_undoable() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item");
    ctx.assert_pch(&["undo"])
        .only_stdout_contains("Undid: add item Test item");
    ctx.assert_pch(&["items"]).is_silent();
}

#[test]
fn adding_an_item_should_match_locations_case_insensitively() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item");
}

#[test]
fn adding_an_item_to_a_nonexistent_bin_should_fail() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch_fails(&["add", "Nonexistent/4", "Test item"])
        .only_stderr_matches("Error: .* \"Nonexistent\"");
}

#[test]
fn adding_items_should_default_to_small_size() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item (S)");
}
//
#[test]
fn adding_items_should_respect_the_given_size() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item", "M"])
        .only_stdout_contains("Test/4: Test item (M)");
}

#[test]
fn items_should_sort_by_location_then_bin_then_alphabetically() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "test/4", "Test item", "M"]);
    ctx.assert_pch(&["add", "test/3", "Test item", "M"]);
    ctx.assert_pch(&["add", "huge/6", "Test item", "M"]);
    ctx.assert_pch(&["add", "test/4", "Test blight'em", "M"]);

    ctx.assert_pch(&["items"]).only_stdout_matches(
        "Huge/6: Test item.*
Test/3: Test item.*
Test/4: Test blight'em.*
Test/4: Test item",
    );
}

#[test]
fn items_should_be_deletable() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "Test/4", "Test item"])
        .only_stdout_contains("Test/4: Test item");
    ctx.assert_pch(&["add", "Test/1", "Don't delete me"])
        .only_stdout_contains("Test/1: Don't delete me");
    ctx.assert_pch(&["delete Test"])
        .only_stdout_contains("Deleted Test/4: Test item");
    ctx.assert_pch(&["items"])
        .only_stdout_matches("Test/4: Test item");
}
