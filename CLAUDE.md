# CLAUDE.md

Guidance for Claude Code (and humans) working in this repository.

## What this is

`lancer-foundry` — A Rust clone of the Foundry Ethereum development toolkit, built as part of the Lancer ecosystem — a complete 2026 AI-era bug bounty toolkit. It reimplements the core Foundry tools (forge, cast, anvil, chisel) in Rust with a focus on programmatic use in automated security research and bug-bounty workflows.

## Architecture

- `src/main.rs` — binary entry point; routes CLI subcommands to subsystems
- `src/forge/` — smart contract compiler, test runner, fuzzer, script executor, ABI/linting tools (`compiler.rs`, `runner.rs`, `fuzz.rs`, `invariant.rs`, `script.rs`, `lint.rs`, `reporter.rs`, `filter.rs`, `doc.rs`)
- `src/cast/` — Ethereum wallet/signing operations and JSON-RPC client utilities (`abi.rs`, `convert.rs`, `rpc.rs`, `sign_ops.rs`, `wallet.rs`)
- `src/anvil/` — local EVM node with HTTP JSON-RPC server; state management and Ethereum API surface (`backend.rs`, `eth_api.rs`, `anvil_api.rs`, `rpc.rs`, `state.rs`, `accounts.rs`)
- `src/chisel/` — interactive Solidity REPL using `rustyline` readline (`repl.rs`, `session.rs`)
- `src/config/` — shared configuration loading (`mod.rs`)
- `src/cheatcodes/` — EVM cheatcode implementations used by the test runner
- `src/evm/` — low-level EVM execution layer wrapping `revm`

## Conventions

- Error propagation uses `eyre::Result` / `color-eyre`; no `unwrap`/`expect` on fallible I/O.
- Async runtime is `tokio` with `features = ["full"]`; CPU-bound work uses `rayon`.
- Library code lives in per-module `mod.rs` files under `src/`; binary entry point in `src/main.rs`.
- CLI surface is built with `clap` derive macros; new subcommands follow the existing `clap` derive pattern.
- Ethereum primitives use the `alloy-*` crates (`alloy-primitives`, `alloy-dyn-abi`, `alloy-rlp`); avoid raw byte manipulation without them.
- Serialisation uses `serde` derive; configuration files are TOML, wire formats are JSON.

## Build / test / run

```
build.bat                   # Windows → build\lancer-foundry.exe
build-windows.bat           # Windows packaged dist → dist\lancer-foundry-windows-x86_64\bin\lancer-foundry.exe
./build-linux.sh            # Linux/macOS → build/lancer-foundry
cargo build --release       # standard release build
cargo test                  # unit + integration tests
```

## Repo notes

- `plan.md` is local-only and must NOT be committed. It is gitignored; if ever tracked, run `git rm --cached plan.md`.
- `.claude/` and `target/`/`build/`/`dist/` are local-only and gitignored.
