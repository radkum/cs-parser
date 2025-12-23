# ps-parser

[![Crates.io](https://img.shields.io/crates/v/ps-parser.svg)](https://crates.io/crates/ps-parser)
[![Crates.io](https://img.shields.io/crates/d/ps-parser.svg)](https://crates.io/crates/ps-parser)
[![Docs.rs](https://docs.rs/ps-parser/badge.svg)](https://docs.rs/ps-parser)
[![License](https://img.shields.io/crates/l/ps-parser.svg)](LICENSE)

A CSharp parser written in Rust.
Parse, evaluate and deobfuscate CSharp scripts with idiomatic Rust types.

## Goal

Malicious scripts typically use "safe" operations to obfuscate "unsafe" ones. For example, arithmetic operations are used to obfuscate function arguments.

The goal of this parser is to combat obfuscation in CSharp by evaluating everything that is "safe" but not anything that is "unsafe". Ps-parser deliver also possibility to get script "tokens"

## Features

- CSharp script parsing using [pest](https://pest.rs/) grammar
- Value types for CSharp objects (`String`, `Int`, `HashTable`, `StringBuilder`, etc.)
- Arithmetic, logical, and string operations
- Script block evaluation and variable management
- HashTable and Array support
- Extensible for custom CSharp types

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
ps-parser = "0.1.0-pre"
```

## Usage

### Parse a CSharp program and eval only "safe" operations

```rust
use cs_parser::CSharpSession;

let mut cs = CSharpSession::new(); 
let script = r#"
namespace Some {
    class Main {
        public static string SUSPICIOUS = Main.InitText("concatenated", "string");
        public static string InitText(string prefix, string suffix) {
            return prefix + "_" + suffix;
        }
    }
}
"#;

let program_res = p.parse_input(input).unwrap();
println!("{:?}", program_res.tokens().string_set());
```

Output: 
```CSharp
{"_", "concatenated", "concatenated_string", "string"}
```

## Future plans
- fix class objects
- implement typeof()

## Documentation

- [API Reference (docs.rs)](https://docs.rs/cs-parser)
- [Crate on crates.io](https://crates.io/crates/cs-parser)

## License

Licensed under MIT or Apache-2.0, at your option.
See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.