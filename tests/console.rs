#[macro_use]
mod common;
use common::*;

use rexpect::session::spawn_command;

#[test]
fn console_has_help() -> rexpect::errors::Result<()> {
    init!(ctx);
    ctx.populate();

    let mut p = spawn_command(ctx.pch_cmd(&["console"]), Some(1000))?;
    p.exp_string("pachinko> ")?;
    p.send_line("help")?;
    p.exp_regex(r"Available commands:")?;
    p.exp_string("add")?;
    p.exp_string("add-location")?;
    p.exp_string("console")?;
    p.exp_string("items")?;
    p.exp_string("locations")?;
    p.exp_string("quickadd")?;

    p.process.exit()?;

    Ok(())
}

#[test]
fn console_can_add_items() -> rexpect::errors::Result<()> {
    init!(ctx);
    ctx.populate();

    let mut p = spawn_command(ctx.pch_cmd(&["console"]), Some(1000))?;
    p.exp_string("pachinko> ")?;
    p.send_line("add Test First")?;
    p.exp_regex("Test.*First")?;

    p.exp_string("pachinko> ")?;
    p.send_line("items")?;
    p.exp_regex("Test.*First")?;

    p.exp_string("pachinko> ")?;

    p.process.exit()?;

    Ok(())
}

#[test]
fn console_can_add_items_with_spaces() -> rexpect::errors::Result<()> {
    init!(ctx);
    ctx.populate();

    let mut p = spawn_command(ctx.pch_cmd(&["console"]), Some(1000))?;
    p.exp_string("pachinko> ")?;
    p.send_line("add Test \"Spacey item\"")?;
    p.exp_regex("Test.*Spacey item")?;

    p.exp_string("pachinko> ")?;
    p.send_line("add Test 'Single spacey item'")?;
    p.exp_regex("Test.*Single spacey item")?;

    p.exp_string("pachinko> ")?;

    p.process.exit()?;

    Ok(())
}

#[test]
fn console_continues_after_bad_commands() -> rexpect::errors::Result<()> {
    init!(ctx);
    ctx.populate();

    let mut p = spawn_command(ctx.pch_cmd(&["console"]), Some(1000))?;
    p.exp_string("pachinko> ")?;
    p.send_line("add Test \"Spacey item")?;
    p.exp_regex(r"Error: .*quote")?;

    p.exp_string("pachinko> ")?;
    p.send_line("ad Test \"Spacey item\"")?;
    p.exp_regex(r"error: .*ad")?;

    p.exp_regex(r"(?s).*?pachinko>")?;
    p.send_line("add Tes \"Spacey item\"")?;
    p.exp_regex(r"Error: .*Tes")?;

    p.exp_regex(r"(?s).*?pachinko>")?;
    p.send_line("add Test \"Spacey item\"")?;
    p.exp_regex("Test.*Spacey item")?;

    p.exp_string("pachinko> ")?;

    p.process.exit()?;

    Ok(())
}
