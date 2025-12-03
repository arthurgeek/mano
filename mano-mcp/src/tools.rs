use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

/// Executes mano code by shelling out to the mano CLI.
/// Returns the combined stdout and stderr output.
pub async fn run_mano_code(mano_bin: &Path, code: &str) -> String {
    // Run mano CLI with stdin
    let mut child = match Command::new(mano_bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return format!("Error executing mano: {}", e),
    };

    // Write code to stdin
    if let Some(mut stdin) = child.stdin.take()
        && let Err(e) = stdin.write_all(code.as_bytes()).await
    {
        return format!("Error writing to stdin: {}", e);
    }

    // Wait for output
    let output = match child.wait_with_output().await {
        Ok(o) => o,
        Err(e) => return format!("Error waiting for mano: {}", e),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stderr.is_empty() {
        stdout.to_string()
    } else if stdout.is_empty() {
        stderr.to_string()
    } else {
        format!("{}\n{}", stdout, stderr)
    }
}

const KEYWORD_REFERENCE: &str = r#"# mano Language Reference

mano is a programming language where all keywords are mano slang.

## Keywords

| Concept | mano | Example |
|---------|------|---------|
| variable | `seLiga` | `seLiga x = 42;` |
| print | `salve` | `salve "oi";` |
| true | `firmeza` | `sePá (firmeza) { ... }` |
| false | `treta` | `sePá (treta) { ... }` |
| nil | `nadaNão` | `seLiga x = nadaNão;` |
| if | `sePá` | `sePá (x > 0) { salve "positive"; }` |
| else | `vacilou` | `sePá (x > 0) { ... } vacilou { ... }` |
| while | `segueOFluxo` | `segueOFluxo (x < 10) { x = x + 1; }` |
| for | `seVira` | `seVira (seLiga i = 0; i < 10; i = i + 1) { salve i; }` |
| break | `saiFora` | `saiFora;` |
| and | `tamoJunto` | `firmeza tamoJunto firmeza` |
| or | `ow` | `treta ow firmeza` |
| function | `olhaEssaFita` | `olhaEssaFita soma(a, b) { toma a + b; }` |
| return | `toma` | `toma x * 2;` |
| class | `bagulho` | `bagulho Pessoa { }` |
| this | `oCara` | `oCara.nome = "João";` |
| super | `mestre` | `mestre.metodo();` |

## Native Functions

| Function | Description | Example |
|----------|-------------|---------|
| `fazTeuCorre()` | Returns current time in seconds | `seLiga tempo = fazTeuCorre();` |
| `viraTexto(x)` | Converts any value to string | `seLiga s = viraTexto(42);` |

## String Interpolation

Strings support interpolation with `{expression}`:

```mano
seLiga nome = "mano";
seLiga idade = 25;
salve "E aí, {nome}! Tu tem {idade} anos.";
// Output: E aí, mano! Tu tem 25 anos.

// Works with any expression
salve "1 + 2 = {1 + 2}";
// Output: 1 + 2 = 3

// Escape with {{ for literal braces
salve "Chaves: {{assim}}";
// Output: Chaves: {assim}
```

## Classes and Inheritance

```mano
// Define a class
bagulho Animal {
    falar() { salve "..."; }
}

// Inheritance with <
bagulho Cachorro < Animal {
    falar() { salve "Au au!"; }
    latir() { mestre.falar(); }  // Call superclass method
}

// Constructor is called "bora"
bagulho Pessoa {
    bora(nome) { oCara.nome = nome; }
}
seLiga p = Pessoa("João");
```

## Operators

- Arithmetic: `+`, `-`, `*`, `/`, `%`
- Comparison: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Logical: `tamoJunto` (and), `ow` (or), `!` (not)
- Ternary: `condition ? then : else`

## Syntax

- Statements end with `;`
- Strings use double quotes: `"hello"`
- Comments: `//` or `/* ... */`
"#;

/// Returns a prompt with mano keyword reference and the code to translate.
pub fn get_translation_prompt(code: &str) -> String {
    format!(
        "{}\n---\n\n## Code to Translate\n\n```\n{}\n```\n\nTranslate the above code to mano. Use `run_mano` to verify!",
        KEYWORD_REFERENCE, code
    )
}
