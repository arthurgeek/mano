mod completer;
mod report;
mod state;

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use mano::{Mano, ManoError};
use rustyline::Editor;
use rustyline::error::ReadlineError;

use completer::ManoHelper;
use report::report_error;
use state::ReplState;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut mano = Mano::new();

    let result = match args.len() {
        0 => run_repl(&mut mano),
        1 => run_file(&mut mano, Path::new(&args[0])),
        _ => {
            eprintln!("Uso: mano [script]");
            return ExitCode::from(64);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(65)
        }
    }
}

fn run_file(mano: &mut Mano, path: &Path) -> Result<(), ManoError> {
    let source = fs::read_to_string(path)?;
    let filename = path.to_string_lossy();
    let errors = mano.run(&source, std::io::stdout());
    for error in &errors {
        report_error(error, &source, Some(&filename), std::io::stderr());
    }
    Ok(())
}

fn run_repl(mano: &mut Mano) -> Result<(), ManoError> {
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
                    let source = if ReplState::should_auto_print(&buffer) {
                        ReplState::wrap_for_print(&buffer)
                    } else {
                        buffer
                    };
                    let errors = mano.run(&source, std::io::stdout());
                    for error in &errors {
                        report_error(error, &source, None, std::io::stderr());
                    }

                    // Update completions with current variables
                    if let Some(helper) = rl.helper() {
                        helper.set_variables(mano.variable_names());
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
