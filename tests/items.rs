#[macro_use]
mod common;
use common::*;

#[test]
fn items_should_be_searchable() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "test/4", "Test item", "M"]);
    ctx.assert_pch(&["add", "huge/6", "Huge item", "M"]);
    ctx.assert_pch(&["add", "test/4", "Test blight'em", "M"]);

    ctx.assert_pch(&["items", "item"]).only_stdout_matches(
        r"Huge/6: Huge item \(M\)
Test/4: Test item",
    );
}

#[test]
fn items_should_sort_by_numeric_bin() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch(&["add", "huge/6", "Huge item", "M"]);
    ctx.assert_pch(&["add", "huge/16", "Huge far item", "M"]);

    ctx.assert_pch(&["items", "item"]).only_stdout_matches(
        r"Huge/6: Huge item \(M\)
Huge/16: Huge far item \(M\)",
    );
}
