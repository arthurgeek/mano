# mano Interpreter Design

A Rust implementation of the "Crafting Interpreters" tree-walk interpreter using São Paulo "mano" slang for all keywords.

## Language Identity

- **Pure mano slang**: Every keyword is authentic paulistano gíria
- **Humor-first**: Keyword mappings prioritize funny/absurd over literal translations
- **Error messages roast**: Mistakes get called out in full mano style
- **UTF-8 native**: Proper Portuguese with accents (`sePá`, `nadaNão`)
- **File extension**: `.mano`
- **Comments**: Standard `//` syntax

## Keywords

| Lox | mano | Meaning |
|-----|------|---------|
| `print` | `salve` / `oiSumida` | "hey!" / "hey stranger!" |
| `var` | `seLiga` | "pay attention" |
| `true` | `firmeza` | "solid/legit" |
| `false` | `treta` | "drama/trouble" |
| `nil` | `nadaNão` | "nothing at all" |
| `and` | `tamoJunto` | "we're together" |
| `or` | `ow` | interjection |
| `if` | `sePá` | "maybe/perhaps" |
| `else` | `vacilou` | "you messed up" |
| `while` | `segueOFluxo` | "follow the flow" |
| `for` | `seVira` | "figure it out" |
| `fun` | `olhaEssaFita` | "check out this story" |
| `return` | `toma` | "take it!" |
| `break` | `saiFora` | "get out" |
| `class` | `bagulho` | "thing/stuff" |
| `this` | `oCara` | "the dude" |
| `super` | `mestre` | "master" |

Note: `salve` and `oiSumida` both work for print (Easter egg).

## Architecture

### Rust Patterns

- **AST**: Enums with `match` instead of visitor pattern
- **Error handling**: `thiserror` crate with `Result` types
- **No garbage collection**: Rust ownership handles memory

### Pipeline

```
Source (.mano) → Scanner → Tokens → Parser → AST → Interpreter → Result
```

### Project Structure

```
mano/
├── Cargo.toml
├── src/
│   ├── main.rs          # CLI: REPL and file runner
│   ├── lib.rs           # Public API, re-exports
│   ├── scanner.rs       # Lexer/tokenizer
│   ├── token.rs         # Token types and keywords
│   ├── ast.rs           # Expr and Stmt enums
│   ├── parser.rs        # Recursive descent parser
│   ├── interpreter.rs   # Tree-walking evaluator
│   ├── environment.rs   # Scope/variable binding
│   └── error.rs         # Error types with mano messages
└── tests/
    └── integration/     # End-to-end .mano file tests
```

## Core Types

### Value (runtime)

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Numero(f64),
    Texto(String),
    Firmeza,          // true
    Treta,            // false
    NadaNao,          // nil
    Funcao(..),       // later
    Bagulho(..),      // later
}
```

### Expressions

```rust
pub enum Expr {
    Literal { value: Value },
    Unary { operator: Token, right: Box<Expr> },
    Binary { left: Box<Expr>, operator: Token, right: Box<Expr> },
    Grouping { expression: Box<Expr> },
    Variable { name: Token },
    Assign { name: Token, value: Box<Expr> },
    // expanded per chapter
}
```

### Statements

```rust
pub enum Stmt {
    Expression { expression: Expr },
    Print { expression: Expr },
    Var { name: Token, initializer: Option<Expr> },
    Block { statements: Vec<Stmt> },
    If { condition: Expr, then_branch: Box<Stmt>, else_branch: Option<Box<Stmt>> },
    // expanded per chapter
}
```

### Errors

```rust
#[derive(Debug, Error)]
pub enum ManoError {
    #[error("Eita mano, linha {line}: {msg}")]
    ScanError { line: usize, msg: String },

    #[error("Aí vacilou! {msg} na linha {line}")]
    ParseError { line: usize, msg: String },

    #[error("Deu ruim na hora H, mano: {msg}")]
    RuntimeError { msg: String },
}
```

## Development Process

### TDD is Mandatory

1. Write a failing test with mano code
2. Implement minimum code to pass
3. Refactor while tests stay green

### Test Patterns

```rust
#[test]
fn test_salve_prints_string() {
    let output = run_mano(r#"salve "E aí, mano!";"#);
    assert_eq!(output, "E aí, mano!\n");
}

#[test]
fn test_error_roasts_user() {
    let error = run_mano_error("seLiga x = ;");
    assert!(error.contains("vacilou"));
}
```

### Implementation Order

Following "Crafting Interpreters" Part II:

1. **Chapter 4**: Scanner - tokenize keywords, literals, operators
2. **Chapter 5**: AST representation (Expr enum)
3. **Chapter 6**: Parser - expressions
4. **Chapter 7**: Interpreter - evaluate expressions
5. **Chapter 8**: Statements, variables (`seLiga`, `salve`)
6. **Chapter 9**: Control flow (`sePá`, `vacilou`, `segueOFluxo`)
7. **Chapter 10**: Functions (`olhaEssaFita`, `toma`)
8. **Chapter 11**: Resolving and binding
9. **Chapter 12**: Classes (`bagulho`, `oCara`, `mestre`)

Before each chapter: discuss Java vs Rust approach, then TDD.

## Reference

Book: https://craftinginterpreters.com/contents.html
