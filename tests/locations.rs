#[macro_use]
mod common;
use common::*;

#[test]
fn there_should_be_no_locations_to_start() {
    init!(ctx);

    ctx.assert_pch(&["locations"]).is_silent();
}

#[test]
fn an_added_location_should_be_visible() {
    init!(ctx);

    ctx.assert_pch(&["add-location", "Test", "16"]).is_silent();
    ctx.assert_pch(&["locations"])
        .only_stdout_contains("Test (16 bins)");
}

#[test]
fn an_added_location_with_one_bin_should_not_include_bins() {
    init!(ctx);

    ctx.assert_pch(&["add-location", "Test", "1"]).is_silent();
    ctx.assert_pch(&["locations"])
        .only_stdout_matches("^Test\n");
}

#[test]
fn creating_a_location_with_an_invalid_number_of_bins_should_fail() {
    init!(ctx);
    ctx.populate();

    ctx.assert_pch_fails(&["add-location", "Zero", "0"]);
    ctx.assert_pch_fails(&["add-location", "Negative", "-1"]);
}
