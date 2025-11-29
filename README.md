# mano 🔥

A tree-walking interpreter implemented in Rust, following the [Crafting Interpreters](https://craftinginterpreters.com/) book. All keywords use São Paulo "mano" slang, and error messages roast you.

**This project exists to learn about interpreters, language design, and Rust.** It's not meant for production use — it's meant for learning and having fun along the way.

## Features

- Full lexer, parser, and tree-walking interpreter
- Brazilian Portuguese keywords with cultural flavor
- REPL with syntax highlighting and auto-complete
- Beautiful error messages using [ariadne](https://github.com/zesterer/ariadne)
- Unicode identifiers (including emoji! `seLiga 🔥 = 100`)
- [Turing complete](examples/minsky.mano) (proven via Minsky machine simulation)

## Example

```
seLiga nome = "Arthur";
salve "E aí, " + nome + "!";

sePá (firmeza) {
    salve "Tá firmeza, mano!";
} vacilou {
    salve "Deu treta...";
}

// FizzBuzz porque agora temos módulo!
seVira (seLiga n = 1; n <= 15; n = n + 1) {
    sePá (n % 15 == 0) salve "FizzBuzz";
    vacilou sePá (n % 3 == 0) salve "Fizz";
    vacilou sePá (n % 5 == 0) salve "Buzz";
    vacilou salve n;
}

// Emoji variables porque sim
oiSumida 🔥;
```

## Keywords

| Lox | mano | Meaning | Status |
|-----|------|---------|--------|
| `print` | `salve` | "hey!" | ✅ |
| `print` | `oiSumida` | "hey stranger!" (alias) | ✅ |
| `var` | `seLiga` | "pay attention" | ✅ |
| `true` | `firmeza` | "solid/legit" | ✅ |
| `false` | `treta` | "drama/trouble" | ✅ |
| `nil` | `nadaNão` | "nothing at all" | ✅ |
| `if` | `sePá` | "maybe/perhaps" | ✅ |
| `else` | `vacilou` | "you messed up" | ✅ |
| `and` | `tamoJunto` | "we're together" | ✅ |
| `or` | `ow` | interjection | ✅ |
| `while` | `segueOFluxo` | "follow the flow" | ✅ |
| `for` | `seVira` | "figure it out" | ✅ |
| `break` | `saiFora` | "get out" | ✅ |
| `fun` | `olhaEssaFita` | "check out this story" | 🔜 |
| `return` | `toma` | "take it!" | 🔜 |
| `class` | `bagulho` | "thing/stuff" | 🔜 |
| `this` | `oCara` | "the dude" | 🔜 |
| `super` | `mestre` | "master" | 🔜 |

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

## Benchmarks (just for fun 😂)

We raced against Node.js. Spoiler: tree-walking interpreter vs V8 JIT goes exactly how you'd expect... eventually.

| Benchmark | mano | Node | Winner |
|-----------|------|------|--------|
| Fibonacci(35) | ~0ms | 40ms | **mano** |
| Primes to 1000 | 7ms | 40ms | **mano** |
| Loop 10k | 9ms | 40ms | **mano** |
| Loop 100k | 47ms | 40ms | Node (barely) |
| Primes to 10k | 59ms | 48ms | Node (barely) |
| Loop 1M | 359ms | 43ms | **Node 8x** |

Plot twist: mano wins on small scripts because Node's JIT warmup (~40ms) is slower than our entire execution! We only lose when the workload is heavy enough for JIT to pay off.

**Conclusion**: If your script runs in under 40ms, just use mano. (Please don't actually do this.)

## License

MIT
