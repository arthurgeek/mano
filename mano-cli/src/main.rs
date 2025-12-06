mod completer;
mod report;
mod state;
mod vm;

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use mano::{Mano, ManoError, Runner};
use rustyline::Editor;
use rustyline::error::ReadlineError;

use completer::ManoHelper;
use report::report_error;
use state::ReplState;
use vm::Vm;

#[derive(Parser)]
#[command(name = "mano")]
#[command(about = "Interpretador da linguagem mano - a linguagem dos cria", long_about = None)]
struct Args {
    /// Script file to execute
    script: Option<PathBuf>,

    /// Use the bytecode VM instead of the tree-walk interpreter
    #[arg(long)]
    vm: bool,

    /// Enable debug tracing (VM mode only)
    #[arg(long)]
    debug: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let result = if args.vm {
        let mut vm = Vm::new();
        vm.set_debug(args.debug);
        run_mode(&mut vm, args.script.as_deref())
    } else {
        let mut mano = Mano::new();
        run_mode(&mut mano, args.script.as_deref())
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let msg = e.to_string();
            if !msg.is_empty() {
                eprintln!("{e}");
            }
            ExitCode::from(65)
        }
    }
}

fn run_mode<R: Runner>(runner: &mut R, script: Option<&Path>) -> Result<(), ManoError> {
    match script {
        Some(path) => run_file(runner, path),
        None => {
            if io::stdin().is_terminal() {
                run_repl(runner)
            } else {
                run_stdin(runner)
            }
        }
    }
}

fn run_file<R: Runner>(runner: &mut R, path: &Path) -> Result<(), ManoError> {
    let source = fs::read_to_string(path)?; // IO errors propagate (will be printed)
    let filename = path.to_string_lossy();
    match runner.run(&source, std::io::stdout()) {
        Ok(()) => Ok(()),
        Err(errors) => {
            for error in &errors {
                report_error(error, &source, Some(&filename), std::io::stderr());
            }
            Err(ManoError::ScriptFailed)
        }
    }
}

fn run_stdin<R: Runner>(runner: &mut R) -> Result<(), ManoError> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?; // IO errors propagate (will be printed)
    match runner.run(&source, std::io::stdout()) {
        Ok(()) => Ok(()),
        Err(errors) => {
            for error in &errors {
                report_error(error, &source, None, std::io::stderr());
            }
            Err(ManoError::ScriptFailed)
        }
    }
}

fn run_repl<R: Runner>(runner: &mut R) -> Result<(), ManoError> {
    let helper = ManoHelper::new();
    let mut rl: Editor<ManoHelper, _> =
        Editor::with_config(rustyline::Config::default()).expect("Falha ao iniciar o REPL, bicho!");
    rl.set_helper(Some(helper));
    let mut state = ReplState::new();

    loop {
        match rl.readline(&state.prompt()) {
            Ok(line) => {
                let _ = rl.add_history_entry(&line);

                if state.process_line(&line) {
                    let buffer = state.take_buffer();
                    let source =
                        if runner.supports_auto_print() && ReplState::should_auto_print(&buffer) {
                            ReplState::wrap_for_print(&buffer)
                        } else {
                            buffer
                        };
                    if let Err(errors) = runner.run(&source, std::io::stdout()) {
                        for error in &errors {
                            report_error(error, &source, None, std::io::stderr());
                        }
                    }

                    // Update completions with current variables
                    if let Some(helper) = rl.helper() {
                        helper.set_variables(runner.variable_names());
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                if state.is_empty() {
                    break;
                }
                state.cancel();
                println!();
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                eprintln!("Deu ruim no REPL, maluco: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn run_file_with_script_error_returns_script_failed() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "@").unwrap();

        let mut mano = Mano::new();
        let result = run_file(&mut mano, file.path());

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), ManoError::ScriptFailed),
            "Expected ManoError::ScriptFailed"
        );
    }
}
