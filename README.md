# mano

A programming language interpreter implemented in Rust, following the [Crafting Interpreters](https://craftinginterpreters.com/) book. All keywords use Brazilian "mano" slang, and error messages roast you.

This project exists to learn Rust and have some fun along the way.

## Example

```
seLiga nome = "Arthur";
salve "E aí, " + nome + "!";

sePá (firmeza) {
    salve "Tá firmeza, mano!";
} vacilou {
    salve "Deu treta...";
}
```

## Keywords

| Lox | mano | Meaning |
|-----|------|---------|
| `print` | `salve` | "hey!" |
| `var` | `seLiga` | "pay attention" |
| `true` | `firmeza` | "solid/legit" |
| `false` | `treta` | "drama/trouble" |
| `nil` | `nadaNão` | "nothing at all" |
| `if` | `sePá` | "maybe/perhaps" |
| `else` | `vacilou` | "you messed up" |
| `and` | `tamoJunto` | "we're together" |
| `or` | `ow` | interjection |
| `while` | `segueOFluxo` | "follow the flow" |
| `for` | `seVira` | "figure it out" |
| `fun` | `olhaEssaFita` | "check out this story" |
| `return` | `toma` | "take it!" |
| `class` | `bagulho` | "thing/stuff" |
| `this` | `oCara` | "the dude" |
| `super` | `mestre` | "master" |

## Usage

```bash
# REPL mode
cargo run

# Run a script
cargo run -- script.mano
```

## Development

```bash
cargo test      # Run tests
cargo clippy    # Lint
cargo fmt       # Format
```

## License

MIT
