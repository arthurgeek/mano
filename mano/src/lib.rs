mod ast;
mod environment;
mod error;
mod interpreter;
mod parser;
mod resolver;
mod scanner;
mod token;
mod value;

use std::io::Write;

pub use ast::{Expr, Stmt};
pub use error::ManoError;
pub use parser::Parser;
pub use scanner::{KEYWORDS, Scanner, is_identifier_char, is_identifier_start};
pub use token::{Literal, Token, TokenType};

/// Native functions available in the interpreter
pub const NATIVE_FUNCTIONS: &[&str] = &["fazTeuCorre"];

/// Name of the initializer method (constructor) - called automatically on instantiation
pub const INITIALIZER_NAME: &str = "bora";

pub struct Mano {
    interpreter: interpreter::Interpreter,
}

impl Default for Mano {
    fn default() -> Self {
        Self::new()
    }
}

impl Mano {
    pub fn new() -> Self {
        Self {
            interpreter: interpreter::Interpreter::new(),
        }
    }

    pub fn variable_names(&self) -> Vec<String> {
        self.interpreter.variable_names()
    }

    pub fn run<O: Write>(&mut self, source: &str, mut stdout: O) -> Vec<ManoError> {
        let mut errors = Vec::new();
        let scanner = scanner::Scanner::new(source);

        let mut tokens = Vec::new();
        for result in scanner {
            match result {
                Ok(token) => tokens.push(token),
                Err(e) => errors.push(e),
            }
        }

        if !errors.is_empty() {
            return errors;
        }

        let mut parser = parser::Parser::new(tokens);
        let statements = parser.parse().unwrap();

        errors.extend(parser.take_errors());

        if !errors.is_empty() {
            return errors;
        }

        // Resolve variable bindings
        let resolver = resolver::Resolver::new();
        let resolutions = match resolver.resolve(&statements) {
            Ok(r) => r,
            Err(errs) => {
                return errs;
            }
        };

        self.interpreter.set_resolutions(resolutions);

        for stmt in &statements {
            if let Err(e) = self.interpreter.execute(stmt, &mut stdout) {
                errors.push(e);
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializer_name_is_bora() {
        assert_eq!(INITIALIZER_NAME, "bora");
    }

    #[test]
    fn run_empty_source_returns_no_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("", &mut stdout);
        assert!(errors.is_empty());
    }

    #[test]
    fn run_comment_only_returns_no_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("// só um comentário", &mut stdout);
        assert!(errors.is_empty());
    }

    #[test]
    fn run_valid_statement_returns_no_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("salve 42;", &mut stdout);
        assert!(errors.is_empty());
    }

    #[test]
    fn run_returns_scanner_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("@", &mut stdout);
        assert_eq!(errors.len(), 1);
        if let ManoError::Scan { message, .. } = &errors[0] {
            assert!(message.contains('@'));
        } else {
            panic!("Expected Scan error");
        }
    }

    #[test]
    fn run_returns_multiple_scanner_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("@$", &mut stdout);
        assert_eq!(errors.len(), 2);
        if let ManoError::Scan { message, .. } = &errors[0] {
            assert!(message.contains('@'));
        } else {
            panic!("Expected Scan error");
        }
        if let ManoError::Scan { message, .. } = &errors[1] {
            assert!(message.contains('$'));
        } else {
            panic!("Expected Scan error");
        }
    }

    #[test]
    fn run_executes_print_statement() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        mano.run("salve 1 + 2;", &mut stdout);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "3");
    }

    #[test]
    fn run_returns_parser_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("1 +", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Parse { .. }));
    }

    #[test]
    fn run_returns_runtime_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // Use variable so type error is only caught at runtime
        let errors = mano.run("seLiga x = \"mano\"; salve -x;", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Runtime { .. }));
    }

    #[test]
    fn run_returns_resolution_error_for_literal_type_mismatch() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // Literal type errors are caught at resolution time
        let errors = mano.run("salve -\"mano\";", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Resolution { .. }));
    }

    #[test]
    fn run_returns_multiple_parser_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let errors = mano.run("seLiga = 1; seLiga y", &mut stdout);
        assert!(errors.len() >= 2);
        assert!(errors.iter().all(|e| matches!(e, ManoError::Parse { .. })));
    }

    #[test]
    fn repl_persists_variables_across_runs() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();

        mano.run("seLiga x = 42;", &mut stdout);

        stdout.clear();
        mano.run("salve x;", &mut stdout);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "42");
    }

    #[test]
    fn errors_dont_affect_subsequent_runs() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();

        // First run has error
        let errors = mano.run("@", &mut stdout);
        assert_eq!(errors.len(), 1);

        // Second run should work fine (no reset needed)
        stdout.clear();
        let errors = mano.run("salve 42;", &mut stdout);
        assert!(errors.is_empty());
        assert_eq!(String::from_utf8(stdout).unwrap().trim(), "42");
    }

    #[test]
    fn default_creates_new_mano() {
        let _mano: Mano = Default::default();
    }

    #[test]
    fn run_returns_resolution_error_for_self_reference() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // seLiga a = a; -> self-reference in initializer
        let errors = mano.run("{ seLiga a = a; }", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Resolution { .. }));
    }

    #[test]
    fn run_returns_resolution_error_for_duplicate_var() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // Two vars with same name in same scope (use underscore prefix to avoid unused var error)
        let errors = mano.run("{ seLiga _x = 1; seLiga _x = 2; }", &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Resolution { .. }));
    }

    // === closure semantics tests ===

    #[test]
    fn closure_captures_variable_at_definition_time() {
        // This is the key test from Chapter 11 - the closure bug
        // The closure should capture 'a' from the enclosing scope
        // where it was DEFINED, not where it's CALLED.
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            seLiga a = "global";
            {
                olhaEssaFita mostra() {
                    salve a;
                }

                mostra();
                seLiga _a = "block";
                mostra();
            }
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        // Both calls should print "global" because the closure
        // was resolved to the outer 'a', not the inner shadowing '_a'
        assert_eq!(lines, vec!["global", "global"]);
    }

    #[test]
    fn closure_captures_enclosing_scope_correctly() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            olhaEssaFita makeCounter() {
                seLiga i = 0;
                olhaEssaFita count() {
                    i = i + 1;
                    salve i;
                }
                toma count;
            }

            seLiga counter = makeCounter();
            counter();
            counter();
            counter();
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines, vec!["1", "2", "3"]);
    }

    #[test]
    fn nested_closures_resolve_correctly() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            seLiga x = "outer";
            olhaEssaFita outer() {
                seLiga x = "middle";
                olhaEssaFita inner() {
                    salve x;
                }
                inner();
            }
            outer();
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "middle");
    }

    // === class declaration tests (Chapter 12.2) ===

    #[test]
    fn class_declaration_prints_bagulho() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Pessoa {}
            salve Pessoa;
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "<bagulho Pessoa>");
    }

    #[test]
    fn class_declaration_with_methods() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Carro {
                buzinar() {
                    toma "Beep!";
                }
            }
            salve Carro;
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "<bagulho Carro>");
    }

    #[test]
    fn class_persists_across_runs() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();

        // Declare class
        let errors = mano.run("bagulho Pessoa {}", &mut stdout);
        assert!(errors.is_empty());

        // Print it in a second run
        stdout.clear();
        let errors = mano.run("salve Pessoa;", &mut stdout);
        assert!(errors.is_empty());
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "<bagulho Pessoa>");
    }

    // === instance creation tests (Chapter 12.3) ===

    #[test]
    fn instance_creation_prints_parada() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Pessoa {}
            salve Pessoa();
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "<parada Pessoa>");
    }

    #[test]
    fn instance_creation_with_variable() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Carro {}
            seLiga meuCarro = Carro();
            salve meuCarro;
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        assert_eq!(output.trim(), "<parada Carro>");
    }

    #[test]
    fn instance_creation_with_arguments_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Pessoa {}
            Pessoa(1, 2);
        "#;
        let errors = mano.run(code, &mut stdout);
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], ManoError::Runtime { .. }));
        if let ManoError::Runtime { message, .. } = &errors[0] {
            assert!(message.contains("0 lances"));
        }
    }

    // === method tests (Chapter 12.5) ===

    #[test]
    fn method_call_returns_value() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Pessoa {
                falar() { toma "oi"; }
            }
            seLiga p = Pessoa();
            salve p.falar();
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        assert_eq!(String::from_utf8(stdout).unwrap().trim(), "oi");
    }

    #[test]
    fn field_shadows_method() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Pessoa {
                falar() { toma "method"; }
            }
            seLiga p = Pessoa();
            p.falar = "field";
            salve p.falar;
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        assert_eq!(String::from_utf8(stdout).unwrap().trim(), "field");
    }

    #[test]
    fn o_cara_returns_instance() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Pessoa {
                getSelf() { toma oCara; }
            }
            seLiga p = Pessoa();
            salve p.getSelf();
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        assert_eq!(String::from_utf8(stdout).unwrap().trim(), "<parada Pessoa>");
    }

    #[test]
    fn o_cara_outside_class_is_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = "salve oCara;";
        let errors = mano.run(code, &mut stdout);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(
            |e| matches!(e, ManoError::Resolution { message, .. } if message.contains("oCara"))
        ));
    }

    #[test]
    fn o_cara_in_function_is_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = "olhaEssaFita teste() { salve oCara; }";
        let errors = mano.run(code, &mut stdout);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(
            |e| matches!(e, ManoError::Resolution { message, .. } if message.contains("oCara"))
        ));
    }

    #[test]
    fn bora_is_called_on_instantiation() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Pessoa {
                bora(nome) {
                    oCara.nome = nome;
                }
            }
            seLiga p = Pessoa("João");
            salve p.nome;
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        assert_eq!(String::from_utf8(stdout).unwrap().trim(), "João");
    }

    #[test]
    fn bora_returns_instance() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Teste {
                bora() {
                    oCara.valor = 42;
                }
            }
            seLiga t = Teste();
            salve t.valor;
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        assert_eq!(String::from_utf8(stdout).unwrap().trim(), "42");
    }

    #[test]
    fn class_arity_matches_bora_params() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        let code = r#"
            bagulho Pessoa {
                bora(nome, idade) {
                    oCara.nome = nome;
                    oCara.idade = idade;
                }
            }
            seLiga p = Pessoa("Maria", 30);
            salve p.nome;
            salve p.idade;
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        assert!(output.contains("Maria"));
        assert!(output.contains("30"));
    }

    #[test]
    fn bora_early_return_still_returns_instance() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // Early return from bora should still return the instance
        let code = r#"
            bagulho Pessoa {
                bora() {
                    toma;
                }
            }
            seLiga p = Pessoa();
            salve p;
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(errors.is_empty(), "Got errors: {:?}", errors);
        let output = String::from_utf8(stdout).unwrap();
        assert!(
            output.contains("<parada Pessoa>"),
            "Expected instance, got: {}",
            output
        );
    }

    #[test]
    fn bora_returning_value_is_error() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // Returning a value from bora should be an error
        let code = r#"
            bagulho Pessoa {
                bora() {
                    toma 42;
                }
            }
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(
            !errors.is_empty(),
            "Expected error for returning value from bora"
        );
        let error_msg = format!("{:?}", errors);
        assert!(
            error_msg.contains("bora") || error_msg.contains("toma"),
            "Error should mention bora or toma: {}",
            error_msg
        );
    }

    #[test]
    fn bora_wrong_arity_errors() {
        let mut mano = Mano::new();
        let mut stdout = Vec::new();
        // bora expects 2 args, but we pass 1
        let code = r#"
            bagulho Pessoa {
                bora(nome, idade) {
                    oCara.nome = nome;
                    oCara.idade = idade;
                }
            }
            seLiga p = Pessoa("João");
        "#;
        let errors = mano.run(code, &mut stdout);
        assert!(!errors.is_empty(), "Expected arity error");
        let error_msg = format!("{:?}", errors);
        assert!(
            error_msg.contains("2") && error_msg.contains("1"),
            "Error should mention expected vs actual args: {}",
            error_msg
        );
    }
}
