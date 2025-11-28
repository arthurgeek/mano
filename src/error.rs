use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManoError {
    #[error("Pô, véi! Cadê o arquivo?")]
    Io(#[from] std::io::Error),

    #[error("[linha {line}] E esse '{lexeme}' aí, truta? Tá na nóia?")]
    UnexpectedCharacter { line: usize, lexeme: char },

    #[error("[linha {line}] Tá moscando, Brown? Cadê o fecha aspas!")]
    UnterminatedString { line: usize },

    #[error("[linha {line}] Ô lesado, esqueceu de fechar o comentário!")]
    UnterminatedBlockComment { line: usize },

    #[error("[linha {line}] Deu mole, cumpadi: {message}")]
    Parse { line: usize, message: String },

    #[error("[linha {line}] Deu ruim na execução, brother: {message}")]
    Runtime { line: usize, message: String },
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
    fn unexpected_character_error_roasts_user() {
        let err = ManoError::UnexpectedCharacter {
            line: 3,
            lexeme: '@',
        };
        assert_eq!(
            err.to_string(),
            "[linha 3] E esse '@' aí, truta? Tá na nóia?"
        );
    }

    #[test]
    fn parse_error_roasts_user() {
        let err = ManoError::Parse {
            line: 5,
            message: "Cadê o fecha parênteses?".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "[linha 5] Deu mole, cumpadi: Cadê o fecha parênteses?"
        );
    }

    #[test]
    fn runtime_error_roasts_user() {
        let err = ManoError::Runtime {
            line: 7,
            message: "Só dá pra negar número, chapa!".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "[linha 7] Deu ruim na execução, brother: Só dá pra negar número, chapa!"
        );
    }
}
