use std::ops::Range;
use thiserror::Error;

use crate::value::Value;

#[derive(Debug, Error)]
pub enum ManoError {
    #[error("Pô, véi! Cadê o arquivo?")]
    Io(#[from] std::io::Error),

    #[error("Tá moscando, Brown?")]
    Scan { message: String, span: Range<usize> },

    #[error("Deu mole, maluco!")]
    Parse { message: String, span: Range<usize> },

    #[error("Deu ruim na execução, brother!")]
    Runtime { message: String, span: Range<usize> },

    #[error("Pô, mano! Erro de escopo!")]
    Resolution { message: String, span: Range<usize> },

    #[error("")]
    Break,

    #[error("")]
    Return(Value),

    #[error("")]
    ScriptFailed, // Script errors already reported, just signal failure
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};

    #[test]
    fn io_error_converts_to_mano_error() {
        let io_err = Error::new(ErrorKind::NotFound, "file not found");
        let mano_err: ManoError = io_err.into();
        assert!(matches!(mano_err, ManoError::Io(_)));
    }

    #[test]
    fn io_error_roasts_user() {
        let io_err = Error::new(ErrorKind::NotFound, "file not found");
        let mano_err: ManoError = io_err.into();
        assert_eq!(mano_err.to_string(), "Pô, véi! Cadê o arquivo?");
    }

    #[test]
    fn scan_error_roasts_user() {
        let err = ManoError::Scan {
            message: "E esse '@' aí, truta?".to_string(),
            span: 10..11,
        };
        assert_eq!(err.to_string(), "Tá moscando, Brown?");
    }

    #[test]
    fn parse_error_roasts_user() {
        let err = ManoError::Parse {
            message: "Cadê o fecha parênteses?".to_string(),
            span: 20..25,
        };
        assert_eq!(err.to_string(), "Deu mole, maluco!");
    }

    #[test]
    fn runtime_error_roasts_user() {
        let err = ManoError::Runtime {
            message: "Só dá pra negar número, chapa!".to_string(),
            span: 30..35,
        };
        assert_eq!(err.to_string(), "Deu ruim na execução, brother!");
    }

    #[test]
    fn resolution_error_roasts_user() {
        let err = ManoError::Resolution {
            message: "Já tem uma 'x' aqui, chapa!".to_string(),
            span: 40..45,
        };
        assert_eq!(err.to_string(), "Pô, mano! Erro de escopo!");
    }
}
