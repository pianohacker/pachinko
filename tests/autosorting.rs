#[macro_use]
mod common;
use common::*;

#[test]
fn adding_an_item_without_a_bin_should_place_it_in_a_random_slot() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "test", "Test item"])
        .only_stdout_matches("Test/[1-4]: Test item .*");
}

#[test]
fn items_should_distribute_evenly() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "test", "Test item"]);
    ctx.assert_pch(&["add", "test", "Test item"]);
    ctx.assert_pch(&["add", "test", "Test item"]);
    ctx.assert_pch(&["add", "test", "Test item"]);

    ctx.assert_pch(&["items"]).only_stdout_matches(
        "Test/1: Test item .*
Test/2: Test item .*
Test/3: Test item .*
Test/4: Test item .*",
    );
}

#[test]
fn items_should_distribute_to_the_most_empty_slot() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "test/1", "M", "M"]);
    ctx.assert_pch(&["add", "test/2", "S", "S"]);
    ctx.assert_pch(&["add", "test/3", "L", "L"]);
    ctx.assert_pch(&["add", "test/4", "X", "X"]);

    ctx.assert_pch(&["add", "test", "X2", "X"])
        .only_stdout_contains("Test/2: X2");
    ctx.assert_pch(&["add", "test", "X3", "X"])
        .only_stdout_contains("Test/1: X3");
}

#[test]
fn items_should_distribute_to_the_first_possible_slot() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "test/2", "L", "L"]);

    ctx.assert_pch(&["add", "test", "X1", "X"])
        .only_stdout_contains("Test/1: X1");
    ctx.assert_pch(&["add", "test", "X3", "X"])
        .only_stdout_contains("Test/3: X3");
}
