#[macro_use]
mod common;
use common::*;

use rexpect::session::spawn_command;

#[test]
fn quick_addition_into_random_bins() -> RexpectResult {
    init!(ctx);
    ctx.populate();

    let mut p = spawn_command(ctx.pch_cmd(&["quickadd", "Test"]), Some(1000))?;
    p.exp_string("Test> ")?;
    p.send_line("Test 1")?;
    p.exp_regex(r"Test/[1234]: Test 1 \(S\)")?;

    p.exp_string("Test> ")?;
    p.send_line("Test 2")?;
    p.exp_regex(r"Test/[1234]: Test 2 \(S\)")?;

    p.exp_string("Test> ")?;
    p.send_line("Test 3 M")?;
    p.exp_regex(r"Test/[1234]: Test 3 \(M\)")?;

    p.process.exit()?;

    Ok(())
}

#[test]
fn quick_addition_into_specified_bin() -> RexpectResult {
    init!(ctx);
    ctx.populate();

    let mut p = spawn_command(ctx.pch_cmd(&["quickadd", "Test/4"]), Some(1000))?;
    p.exp_string("Test/4> ")?;
    p.send_line("Test 1")?;
    p.exp_regex(r"Test/[1234]: Test 1 \(S\)")?;

    p.process.exit()?;

    Ok(())
}
