use std::io::{self, BufReader};
use std::path::Path;
use std::process::ExitCode;

use mano::Mano;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut mano = Mano::new();

    let result = match args.len() {
        0 => mano.run_prompt(BufReader::new(io::stdin()), io::stdout()),
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
