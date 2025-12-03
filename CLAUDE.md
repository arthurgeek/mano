# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**mano** is a programming language interpreter implemented in Rust, following the "Crafting Interpreters" book (https://craftinginterpreters.com/). The language uses São Paulo "mano" slang for all keywords, prioritizing humor and cultural authenticity. Error messages roast the user in full mano style.

## Build Commands

```bash
cargo build                          # Build the project
cargo run -p mano-cli                # Run the interpreter (REPL mode)
cargo run -p mano-cli -- <file>.mano # Run a .mano script file
cargo test                           # Run all tests
cargo test <test_name>               # Run a single test
cargo clippy                         # Lint the code
cargo fmt                            # Format the code
cargo tarpaulin --engine llvm --ignore-tests  # Code coverage
```

### Pre-commit Checklist

Before committing, always run:
```bash
cargo fmt && cargo clippy && cargo test
```

## Development Approach

### Test-Driven Development (TDD)

TDD is mandatory for this project. For every feature:
1. Write a failing test first
2. Implement the minimum code to pass
3. Refactor while keeping tests green

### Commits

Use semantic commit messages: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`

### Rust Patterns

- **Before implementing each chapter**: Discuss whether to follow the Java approach or use a Rust-specific pattern
- **AST**: Use Rust enums with `match` instead of the visitor pattern
- **Error handling**: Use `thiserror` crate with `Result` types
- **UTF-8**: Full support for accented Portuguese characters in keywords

## Architecture

```
Source Code → Scanner → Tokens → Parser → AST → Interpreter → Result
```

Modules:
- `scanner.rs`: Lexical analysis - converts source text to tokens
- `token.rs`: Token types and data structures
- `ast.rs`: Expression and statement enum definitions
- `parser.rs`: Parses tokens into AST
- `interpreter.rs`: Tree-walking interpreter
- `environment.rs`: Variable scope and binding management
- `error.rs`: Error types with mano-style messages

## Language Keywords

UTF-8 with proper Portuguese accents. Comments use `//`.

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
| `init` | `bora` | "let's go!" (initializer) |
| `super` | `mestre` | "master" |

## Book Reference

https://craftinginterpreters.com/contents.html - Part II (Tree-Walk Interpreter)
