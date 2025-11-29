use rexpect::error::Error;
use rexpect::spawn;

fn mano_repl() -> Result<rexpect::session::PtySession, Error> {
    spawn("cargo run --quiet", Some(30_000))
}

#[test]
fn ctrl_c_on_empty_prompt_exits() -> Result<(), Error> {
    let mut p = mano_repl()?;

    // Wait for prompt
    p.exp_string("> ")?;

    // Send Ctrl+C on empty prompt - should exit
    p.send_control('c')?;

    // Process should exit
    p.exp_eof()?;

    Ok(())
}

#[test]
fn ctrl_c_mid_block_cancels_and_continues() -> Result<(), Error> {
    let mut p = mano_repl()?;

    // Wait for prompt
    p.exp_string("> ")?;

    // Start a block
    p.send_line("{")?;

    // Wait for continuation prompt
    p.exp_string("..1 ")?;

    // Send Ctrl+C - should cancel block and show fresh prompt
    p.send_control('c')?;

    // Should get fresh prompt (not continuation)
    p.exp_string("> ")?;

    // Now we can type a normal statement
    p.send_line("salve 42;")?;

    // Should output 42
    p.exp_string("42")?;

    // Exit cleanly
    p.send_control('c')?;
    p.exp_eof()?;

    Ok(())
}

#[test]
fn ctrl_d_exits_repl() -> Result<(), Error> {
    let mut p = mano_repl()?;

    // Wait for prompt
    p.exp_string("> ")?;

    // Send Ctrl+D (EOF)
    p.send_control('d')?;

    // Process should exit
    p.exp_eof()?;

    Ok(())
}
