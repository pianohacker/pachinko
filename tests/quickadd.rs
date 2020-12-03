#[macro_use]
mod common;
use common::*;

#[test]
fn quick_addition_into_random_bins() {
    init!(ctx);
    ctx.populate();

    ctx.pch_cmd(&["quickadd", "Test"])
        .write_stdin(
            "Test 1
Test 2
Test 3 M",
        )
        .assert()
        .success()
        .only_stdout_matches(
            r"Test/[1234]: Test 1 \(S\)
Test/[1234]: Test 2 \(S\)
Test/[1234]: Test 3 \(M\)",
        );
}
