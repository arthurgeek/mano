use std::path::Path;
use std::process::ExitCode;

use mano::Mano;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut mano = Mano::new();

    let result = match args.len() {
        0 => run_repl(&mut mano),
        1 => mano.run_file(Path::new(&args[0])),
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

fn run_repl(mano: &mut Mano) -> Result<(), mano::ManoError> {
    let mut rl = DefaultEditor::new().expect("Falha ao iniciar o REPL, bicho!");

    loop {
        match rl.readline("> ") {
            Ok(line) => {
                let _ = rl.add_history_entry(&line);
                mano.reset_error();
                mano.run(&line)?;
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C - exit
                break;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D - exit
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
