use thiserror::Error;

#[derive(Debug, Error)]
pub enum ManoError {
    #[error("Aí vacilou, mano! Cadê o arquivo?")]
    Io(#[from] std::io::Error),

    #[error("[linha {line}] E esse '{lexeme}' aí, mano? Tá inventando?")]
    UnexpectedCharacter { line: usize, lexeme: char },

    #[error("[linha {line}] Tá moscando, Brown? Cadê o fecha aspas!")]
    UnterminatedString { line: usize },

    #[error("[linha {line}] Ô lesado, esqueceu de fechar o comentário!")]
    UnterminatedBlockComment { line: usize },
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
        assert_eq!(mano_err.to_string(), "Aí vacilou, mano! Cadê o arquivo?");
    }

    #[test]
    fn unexpected_character_error_roasts_user() {
        let err = ManoError::UnexpectedCharacter {
            line: 3,
            lexeme: '@',
        };
        assert_eq!(
            err.to_string(),
            "[linha 3] E esse '@' aí, mano? Tá inventando?"
        );
    }
}
