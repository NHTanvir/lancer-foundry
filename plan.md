# RESEARCH DOSSIER: Foundry

> Part of the Lancer ecosystem. A Rust clone of Paradigm's Foundry — the de-facto Ethereum smart contract development toolkit. Build with Claude Code.
> Upstream: github.com/foundry-rs/foundry (Rust, v1.3.x May 2026, v1.0 shipped 2025-02-13). v1.4 nightly previews reth-anvil + coverage-guided fuzzing.

## Feature Parity Mandate

**`lancer-foundry` MUST ship with 100% feature parity to the upstream `foundry` tool — every flag, every subcommand, every output format, every probe / detector / module / payload / mode — plus a native Rust GUI on top.** This is not a "subset clone" — this is a full reimplementation in Rust with a GUI wrapper.

### Parity contract

- **Every CLI flag** documented in upstream `foundry --help` (and the long-form man page / docs) must work in `lancer-foundry --cli`.
- **Every output format** the upstream tool emits (text, JSON, XML, CSV, SARIF, custom formats) must round-trip through `lancer-foundry` byte-equivalent where the format allows.
- **Every probe / detector / module / category / payload class / built-in script** must be ported. None silently dropped.
- **Every supported target type / input format / protocol / file format / language** must work.
- **Every configuration option** (config files, env vars, registry / settings entries) must be honoured.
- **Companion binaries / sub-tools** in the upstream suite (if any) must each get their own binary in this workspace.

### GUI on top

Every feature reachable via CLI is also reachable from the GUI (see `## GUI (Lancer-standard pattern)` below). The GUI is not a subset — it is an alternate frontend over the same `core` library that the CLI uses. The `core` crate is GUI-agnostic.

### Verification

- The `## Granular feature catalog` section below enumerates everything that must be supported.
- Reference: `tests/upstream_parity/` will pin upstream flag combinations + canned outputs as parity tests. CI fails if any flag is unsupported or output diverges.
- A `lancer-foundry --self-test` flag runs the full parity matrix against a bundled reference target and prints pass/fail per upstream feature group.

### What is allowed to diverge

- **Performance** — Rust-native, async-first; we are allowed to be faster.
- **Memory footprint** — we are allowed to be smaller.
- **GUI** — entirely new (no upstream GUI to copy except where the upstream ships its own GUI like Zenmap / Wireshark UI / Ghidra GUI / jadx-gui — in which case our GUI must cover every panel the upstream GUI has, plus the Lancer-standard layout from `## GUI` below).
- **AI augmentation** — net-new (see `## AI augmentation ideas`).

### What is NOT allowed to diverge

- Any flag silently dropped, renamed, or behavior-changed compared to upstream.
- Any output format / record schema diverging in field names, types, or semantics.
- Any built-in script / detector / module not ported (mark explicitly with `[N/A — see note]` if genuinely impossible in Rust, e.g. NSE Lua scripts requiring features not in `mlua`).

## What it actually does

Foundry is Paradigm's Rust-based Ethereum development toolkit — the de facto standard for smart contract dev/test/deploy as of 2025-2026. It bundles four binaries:
- **forge** — build, test (unit/fuzz/invariant), script/deploy, gas snapshots, linting, coverage, docs
- **cast** — Swiss-army CLI for EVM RPC, ABI codec, wallet ops, transaction crafting, signature verification, on-chain debugging
- **anvil** — local devnet (geth/reth-style) with deep fork support and cheat RPC endpoints
- **chisel** — Solidity REPL on top of revm

Shipped v1.0 on 2025-02-13 (Paradigm announcement). Current line as of 2026-05 is v1.3.x with v1.4 nightly previewing reth-anvil and coverage-guided fuzzing.

## Internal architecture

**Workspace layout** (`crates/`) — 23 crates:

| Crate | Purpose |
|---|---|
| `forge` | Main test runner; subcommand dispatcher |
| `cast` | RPC + ABI CLI |
| `anvil` | Devnet server (axum + tokio) |
| `chisel` | REPL |
| `evm` | Wrapper around revm providing inspectors, executor, traces |
| `cheatcodes` | All ~250 cheatcode implementations + `spec/` sub-crate which generates the `Vm.sol` interface |
| `config` | Foundry.toml parsing, profile resolution |
| `common` | Shared utilities (RPC, fs, console) |
| `cli` | Clap framework wrappers, opts macros |
| `debugger` | TUI debugger using ratatui |
| `doc` | NatSpec → mdBook generator |
| `fmt` | Solidity formatter (own parser) |
| `lint` | Forge-lint (new in v1.3) — solar-based AST lints |
| `linking` | Library linking for compiled artifacts |
| `script` / `script-sequence` | Deploy script orchestration, replay |
| `verify` | Etherscan/Sourcify/Blockscout verification |
| `primitives` | Core types (`Address`, `B256` are re-exports from alloy) |
| `macros` / `sol-macro-gen` | Proc-macros for code generation |
| `test-utils` | Internal test helpers |
| `tui` | Terminal UI shared widgets |

**Shared dependency stack:** `revm` (EVM execution), `alloy` (RPC + types), `foundry-compilers` (out-of-tree crate — wraps solc/svm), `solar` (parser, replacing solang-parser).

**forge test pipeline** (the canonical hot path):

1. `crates/forge/src/cmd/test.rs` parses `--match-*` filters.
2. `foundry-compilers` invokes `solc` (via `svm-rs`); cached artifacts go to `out/` and `cache/`.
3. `crates/forge/src/multi_runner.rs` builds a `MultiContractRunner` with executor per test contract.
4. The executor wraps revm with a stack of `Inspector`s: `Cheatcodes`, `LogCollector`, `Tracer`, `CoverageInspector`, `GasInspector`. revm's `Database` trait points either to in-memory state or a `Backend` that proxies fork RPC.
5. `setUp()` runs first under the same backend. State is snapshotted (revm Journal); each test function reverts back to the snapshot.
6. Fuzz tests use `crates/evm/evm/src/executors/fuzz/` (proptest-like generator); invariant tests use `executors/invariant/` which generates random call sequences and minimizes failures via the shrinker (rewritten in v1.0, "up to 1000x faster").

**Cheatcode dispatch:** when revm hits a `CALL` to address `0x7109709ECfa91a80626fF3989D68f67F5b1DD12D` (the magic vm address), the `Cheatcodes` inspector intercepts. Each cheatcode is identified by 4-byte selector. The `spec/` sub-crate uses procedural macros to derive selectors from a Rust enum and emit the matching `Vm.sol` interface — single source of truth.

**Fork mode internals:** `Backend` (in `crates/evm/core/src/backend/`) implements revm's `Database` trait by lazy-loading account state, storage slots, and block hashes from RPC. Each `createFork()` allocates a new in-memory journal; `selectFork()` swaps the active journal pointer. `makePersistent()` flags an address whose state propagates across fork switches.

## Granular feature list

**forge subcommands** (29): `bind`, `bind-json`, `build`, `cache`, `clone`, `compiler`, `config`, `coverage`, `create`, `debug`, `doc`, `eip712`, `flatten`, `fmt`, `geiger`, `init`, `inspect`, `install`, `lint`, `remappings`, `remove`, `script`, `selectors`, `snapshot`, `soldeer`, `test`, `tree`, `update`, `verify-contract`, `watch`.

**cast subcommands** (~80): `access-list`, `abi-decode`, `abi-encode`, `4byte`, `4byte-decode`, `address-zero`, `age`, `artifact`, `balance`, `base-fee`, `bind`, `block`, `block-number`, `call`, `call-tx`, `chain-id`, `client`, `code`, `codesize`, `completions`, `compute-address`, `concat-hex`, `constructor-args`, `create2`, `creation-code`, `da-estimate` (v1.3), `decode-abi`, `decode-calldata`, `decode-eof`, `decode-string`, `decode-event`, `decode-transaction`, `disassemble`, `erc20`, `estimate`, `etherscan-source`, `find-block`, `format-bytes32-string`, `from-bin`, `from-fixed-point`, `from-rlp`, `from-utf8`, `from-wei`, `gas-price`, `generate-fig-spec`, `hash-message`, `hash-zero`, `index`, `index-erc7201`, `interface`, `keychain` (v1.3), `keccak`, `lookup-address`, `logs`, `max-int`, `max-uint`, `min-int`, `mktx`, `batch-mktx` (v1.3), `nonce`, `parse-bytes32-address`, `parse-bytes32-string`, `pretty-calldata`, `proof`, `publish`, `receipt`, `recover-authority` (v1.3, EIP-7702), `resolve-name`, `rpc`, `run`, `send`, `batch-send` (v1.3), `shl`, `shr`, `sig`, `sig-event`, `storage`, `tempo`, `to-ascii`, `to-base`, `to-bytes32`, `to-check-sum-address`, `to-dec`, `to-fixed-point`, `to-hex`, `to-hexdata`, `to-int256`, `to-rlp`, `to-uint256`, `to-unit`, `to-utf8`, `to-wei`, `trace`, `tx`, `tx-pool`, `upload-signature`, `vaddr`, `wallet new`, `wallet sign`, `wallet vanity`, `wallet address`, `wallet import`, `wallet list`, `wallet decrypt-keystore`.

**anvil flags (selected):** `--fork-url`, `--fork-block-number`, `--fork-chain-id`, `--fork-header`, `--no-storage-caching`, `--chain-id`, `--accounts`, `--mnemonic`, `--balance`, `--gas-limit`, `--gas-price`, `--base-fee`, `--block-time`, `--no-mining`, `--auto-impersonate`, `--code-size-limit`, `--disable-code-size-limit`, `--state` / `--load-state`, `--dump-state`, `--port`, `--host`, `--config-out`, `--prune-history`, `--hardfork`, `--ipc`.
Anvil RPC extras: `anvil_setBalance`, `anvil_setCode`, `anvil_setStorageAt`, `anvil_setNonce`, `anvil_impersonateAccount`, `anvil_mine`, `anvil_setNextBlockTimestamp`, `anvil_dumpState`, `anvil_loadState`, `anvil_dealERC20` (v1.3), `anvil_setERC20Allowance` (v1.3), `anvil_addBalance` (v1.3), `eth_sendRawTransactionSync` (v1.3), `anvil_getBlobByHash` (v1.3).

**chisel commands:** `!help`, `!quit`, `!clear`, `!source`, `!save`, `!load`, `!list`, `!clearcache`, `!fork`, `!traces`, `!calldata`, `!memdump`, `!stackdump`, `!edit`, `!exec`, `!fetch`, `!rawstack`, `!exportreplay`.

**Cheatcodes** (~250 across categories — exhaustive groups from `crates/cheatcodes/spec/src/vm.rs`):

*Block/Tx env:* `warp`, `roll`, `fee`, `coinbase`, `chainId`, `difficulty`, `prevrandao`, `txGasPrice`, `blobhashes`, `getBlobhashes`, `blobBaseFee`, `getBlobBaseFee`, `getBlockNumber`, `getBlockTimestamp`, `getChainId`, `setBlockhash`, `getRawBlockHeader` (v1.3), `executeTransaction`.

*Account state:* `deal`, `etch`, `setNonce`, `setNonceUnsafe`, `resetNonce`, `getNonce`, `addr`, `load`, `store`, `cool`, `warmSlot`, `coolSlot`, `accessList`, `noAccessList`, `cloneAccount`, `getCode`, `getDeployedCode`.

*Prank/caller:* `prank`, `prank(addr,addr)`, `startPrank`, `stopPrank`, `readCallers`, `broadcast`, `startBroadcast`, `stopBroadcast`.

*Mock:* `mockCall`, `mockCalls`, `mockCallRevert`, `mockFunction`, `clearMockedCalls`.

*Expect/assert:* `expectRevert` (many overloads), `expectEmit`, `expectEmitAnonymous`, `expectCall`, `expectCallMinGas`, `expectSafeMemory`, `expectSafeMemoryCall`, `assume`, `assumeNoRevert` (v1.0+), `expectPartialRevert`, `expectNoEmit`.

*Asserts (forge-std):* `assertEq`, `assertNotEq`, `assertTrue`, `assertFalse`, `assertGt`, `assertGe`, `assertLt`, `assertLe`, `assertApproxEqAbs`, `assertApproxEqRel`, `assertApproxEqAbsDecimal`, etc. (typed for every primitive).

*Recording:* `record`, `stopRecord`, `accesses`, `recordLogs`, `getRecordedLogs`, `getRecordedLogsJson`, `startStateDiffRecording`, `stopAndReturnStateDiff`, `getStateDiff`, `getStateDiffJson`, `startDebugTraceRecording`, `stopAndReturnDebugTraceRecording`, `startMappingRecording`, `stopMappingRecording`, `getMappingLength`, `getMappingSlotAt`, `getMappingKeyAndParentOf`, `getStorageSlots`, `getStorageAccesses`.

*Snapshots:* `snapshotState`, `revertToState`, `revertToStateAndDelete`, `deleteStateSnapshot`, `deleteStateSnapshots`, `snapshotValue`, `snapshotGasLastCall`, `startSnapshotGas`, `stopSnapshotGas`, `lastCallGas`, `pauseGasMetering`, `resumeGasMetering`, `resetGasMetering`.

*Forking:* `createFork`, `createSelectFork`, `selectFork`, `activeFork`, `rollFork`, `transact`, `rpc`, `eth_getLogs`, `makePersistent`, `revokePersistent`, `isPersistent`, `allowCheatcodes`, `loadAllocs`, `dumpState`, `getEvmVersion`, `setEvmVersion`.

*Signing:* `sign`, `signCompact`, `signP256` (EIP-7212), `createWallet`, `getWallets`, `rememberKey`, `rememberKeys`, `deriveKey`, `publicKey`, `signDelegation` (EIP-7702), `attachDelegation`, `signAndAttachDelegation`, `signCrossChainDelegation` (v1.3).

*Filesystem (gated by `fs_permissions`):* `readFile`, `readFileBinary`, `writeFile`, `writeFileBinary`, `appendFile`, `removeFile`, `removeDir`, `exists`, `isFile`, `isDir`, `readDir`, `createDir`, `readLine`, `writeLine`, `closeFile`, `fsMetadata`, `projectRoot`, `getCode`, `getDeployedCode`, `unixTime`.

*External commands:* `ffi`, `tryFfi`, `prompt`, `promptSecret`, `promptAddress`, `promptUint`.

*Env vars:* `setEnv`, `envBool`, `envUint`, `envInt`, `envAddress`, `envBytes32`, `envBytes`, `envString`, plus `*Array` and `envOr*` variants.

*JSON/TOML:* `parseJson`, `parseJsonUint`, `parseJsonAddress`, `parseJsonBool`, `parseJsonBytes`, `parseJsonBytes32`, `parseJsonInt`, `parseJsonString`, `parseJsonKeys`, plus `*Array` variants; `serializeJson`, `serialize<Type>`, `writeJson`, `keyExists`. Same for `parseToml*` / `serializeToml*` / `writeToml`.

*String/parse:* `toString`, `parseBytes`, `parseBytes32`, `parseAddress`, `parseUint`, `parseInt`, `parseBool`, `indexOf`, `replace`, `split`, `toUppercase`, `toLowercase`, `trim`, `contains`.

*Utility:* `label`, `getLabel`, `unixTime`, `keyExistsJson`, `keyExistsToml`, `breakpoint`, `pauseTracing`, `resumeTracing`, `skip`, `randomUint`, `randomAddress`, `randomBytes`, `useSeed` (v1.3).

*Misc:* `deployCode`, `getBroadcast`, `replaceEvmVersion`, `setArbitraryStorage`, `copyStorage`.

## Engineering hard parts

**revm plumbing.** Foundry wraps revm with custom `Backend` and `Inspector` types. The hard parts:
- Multi-fork management requires keeping multiple journaled databases alive simultaneously while sharing inspector state.
- The cheatcode inspector must handle re-entrancy — a cheatcode can call into Solidity which can call into another cheatcode.
- `expectRevert` is implemented by wrapping the next sub-call frame: revm's `step`/`step_end` is the hook point; the inspector tags the next CALL frame and intercepts its outcome.

**Cheatcode dispatch.** Selectors are computed at compile time via `sol!` macros from alloy. Each cheatcode is a Rust struct implementing a `Cheatcode` trait with `apply(state, executor) -> Result<Vec<u8>, _>`. The `Cheatcodes` inspector matches the 4-byte selector via a giant match. The `spec/` sub-crate auto-generates the matching `Vm.sol` so Solidity code and Rust dispatcher are kept in lockstep (build-time check).

**Fork mode internals.** `Backend` holds a `BTreeMap<LocalForkId, Fork>`. Each `Fork` has its own `JournaledState`. `selectFork()` swaps which Fork the current `inspect_replay` uses. The hard part is making persistent accounts (test contract, cheatcode dispatcher) appear in every fork while reads/writes to other addresses route to the active fork's RPC. Done by intercepting `basic`, `code_by_hash`, `storage`, `block_hash` calls in the `Backend::Database` impl with an explicit persistent-account check.

**Invariant shrinking** (rewritten in v1.0). The new algorithm performs binary search over call sequence length, then per-call argument minimization, executing the entire sequence each iteration with a cached pre-state. Replay determinism is preserved by deterministic RNG seeded from the input.

**EIP-7702 cheatcodes.** A `SignedAuthorization` struct (RLP-encoded `[chain_id, address, nonce]` with secp256k1 signature) gets attached to the next outgoing transaction's authorization list. revm v25+ honors authorization lists by treating the EOA as having delegated code during the call.

## Library mapping (Rust)

| Need | Crate | Version (v1.3 era) |
|---|---|---|
| EVM execution | `revm` | v27 |
| Types/ABI/RPC | `alloy` (workspace) | 0.x |
| Solc invocation | `foundry-compilers` (own) | 0.x |
| svm | `svm-rs` | 0.x |
| Parser (new) | `solar-parse`, `solar-ast`, `solar-sema` | 0.x — replacing `solang-parser` |
| CLI | `clap` v4 | with derive |
| Async | `tokio` | 1.x |
| Web server (anvil) | `axum` | 0.7+ |
| TUI | `ratatui`, `crossterm` | latest |
| Templating | `handlebars` | 5.x (for doc) |
| Diagnostics | `eyre`, `color-eyre` | |
| Tracing | `tracing`, `tracing-subscriber` | |
| Hashing | `sha2`, `sha3`, `tiny-keccak` | |
| Solver-style fuzz | `proptest` | |
| Parallelism | `rayon`, `futures` | |
| Serialization | `serde`, `serde_json`, `toml` | |

## Validation test corpus

- **forge-std tests** (`lib/forge-std/test/`) — every cheatcode has a matching `*.t.sol` exercising it; this *is* the cheatcode spec.
- **foundry/crates/forge/tests/cli/** — golden tests for every CLI command, JSON output assertions.
- **foundry/testdata/** — sample multi-contract projects with known behaviors.
- **anvil/tests/it/** — RPC integration tests against ephemeral anvil.
- **Known-good behavior fixtures:** Counter.sol from forge-init template, OpenZeppelin contracts compiled clean, Uniswap V2 fork-tests using mainnet block 18,000,000.

## 2024–2026 state

- **v1.0 (Feb 2025):** EIP-7702 cheatcodes (`signDelegation` family), EIP-7212 (`signP256`), EOF support (`--eof` flag, `cast decode-eof`), gas snapshots (`startSnapshotGas`/`stopSnapshotGas`), `assumeNoRevert`, internal call tracing (`--decode-internal`), flamegraphs, `deployCode`, `getBroadcast`, `rememberKeys`/`getWallets`, real-time progress tracker (`--show-progress`), 1000x faster invariant shrinking, 5.2x compile speedup, 2x test speedup, 40% faster fuzzing.
- **v1.2:** Soldeer dependency manager integration, EIP-712 hash utilities, better trace decoding.
- **v1.3 (mid-2025):** forge-lint built-in (`forge lint` subcommand — security lints: reentrancy-events, unprotected-initializer, locked-ether, weak-prng, arbitrary-send-erc20), table tests, time-based invariant campaigns, coverage-guided invariant fuzzing (mutations targeting new paths), cast `da-estimate`, `recover-authority`, `compute-address` (CREATE2), `keychain` diagnostics, `batch-mktx`/`batch-send`, anvil `dealERC20`/`setERC20Allowance`/`eth_sendRawTransactionSync`, `useSeed` cheatcode, `getRawBlockHeader` cheatcode, EIP-7702 cross-chain delegation helpers, revm v27 (21.5% test speedup), reth `eth_getAccountInfo` (5+s faster fork).
- **v1.4 nightly (2026):** session registry scaffolding for Tempo, configurable contract size, AFL showmap corpus replay, machine runtime capability guards, network dispatch in chisel.
- **Roadmap:** reth-anvil unified backend, Solar compiler adoption (replaces solang-parser fully), fuzzing parallelism, native symbolic test runner ("hooks for Kontrol/Halmos").

## Real-world audit workflows

A typical Code4rena/Sherlock/Cantina contest week:
1. `forge init`, import scope, `forge build` to confirm cleanliness.
2. `aderyn .` — generate static report; triage to discard known-FP categories.
3. Read code top-down; mark suspicions in a `findings.md`.
4. For each suspicion, write a forge unit/fork test reproducing the suspected misbehavior.
5. Write `invariant_*` tests for property-level claims, run `forge test --match-test invariant_ -vvv --fuzz-runs 100000`.
6. If invariants survive, port them to Medusa for parallel runs with `medusa fuzz --workers 16 --timeout 14400` overnight.
7. For paths exhibiting numeric edge cases, write `check_*` Halmos tests targeting the same property symbolically.
8. Found bugs go into a finding template referencing forge PoC.

Solodit checklist (380+ items) is used pre-submission to ensure coverage. Categories: front-running, reentrancy (sundry sub-categories), governance/DAO, oracle staleness, MEV, signature replay, low-level call return checks, ERC-20 quirks (USDT no-return, fee-on-transfer, double-entry tokens, blocklists), donation/inflation attacks, share-precision attacks, slot collisions, upgradeability initializers, delegatecall reentrancy, etc.

### Research note — 2026-05-31 (Nightly + 2026 deltas)

Direct walk of `github.com/foundry-rs/foundry/releases` Nightly + recent stable. New items not yet captured:

- **`--introspect` flag (May 2026).** Machine-readable command discovery — `forge test --introspect` emits a JSON schema of available subcommands + flags + types. Used by IDE plugins and our Lancer GUI to auto-generate parameter forms. Our `clap` parser must expose the same JSON via a derive-based reflection layer.
- **Tempo session registry scaffold (May 2026).** Tempo is Foundry's new in-memory session-state primitive for `forge script` runs — `vm.tempoStart()` / `vm.tempoStop()` / `vm.tempoExportJson()`. Persists session state across script invocations. Our cheatcode list adds these three.
- **Tempo fork hardfork preservation (May 2026).** When running `forge test --fork-url <rpc>` with Tempo enabled, the detected hardfork is now preserved across nested vm.createFork() calls. Was reset to default. Our fork-config logic must mirror.
- **Anvil epoch slots handling fix (May 2026).** Anvil's `eth_call` and `evm_setBlockTimestamp` now compute epoch slots consistently. Was off-by-one. Our anvil-equivalent must align.
- **Forge gas optimization for script tests (May 2026).** `forge script` gas estimation now batches similar calls. Major speedup for large script tests. Our script runner mirrors.
- **Polkadot chain support (May 2026).** Nightly adds native `polkadot-testnet`, `polkadot`, `kusama` recognized chain IDs with default RPC endpoints. Our chains DB must include them (separate from EVM mainnet/testnet matrix).
- **forge-std additions (2026).** New helpers landed: `StdInvariant.setUp()` lifecycle, `StdStorage.checked_write_int()` for signed-int slot writes, `StdAssertions.assertAlmostEq(lhs, rhs, maxDelta)` for floating-point-like comparisons in fixed-point ints, `StdJson.write*` family for structured JSON output. Port verbatim.

Sources: [foundry-rs/foundry releases](https://github.com/foundry-rs/foundry/releases), [Foundry Book](https://book.getfoundry.sh/).

## AI augmentation ideas

- **Cheatcode generator.** LLM reads a function signature + natspec spec and emits a forge test using the right `vm.*` cheatcode (e.g., "this needs `vm.warp` and `vm.prank` because…").
- **Test scaffolding from spec.** Given a contract + plain-English spec, generate property invariants and fuzz tests.
- **Counterexample explainer.** When forge fuzz finds a failure, summarize root cause and suggest fix.
- **Auto-decode unknown calldata.** Use 4byte database + LLM to guess function from calldata when no ABI.
- **Trace summarizer.** Convert verbose `forge test -vvvv` output into bullet-point story.

## Implementation phases (10-week clone)

**Week 1 — Skeleton.** Cargo workspace with `forge`, `cast`, `anvil`, `chisel`, `evm`, `cheatcodes`, `config`. Wire up revm (v27), alloy, clap. `cast --version` and `cast wallet new` working.

**Week 2 — cast (read-only RPC).** `cast call`, `cast balance`, `cast block`, `cast code`, `cast storage`, `cast nonce`, `cast tx`, `cast receipt`, `cast logs`. ABI codec: `abi-decode`, `abi-encode`, `sig`, `4byte`, `keccak`, conversions (`to-wei`, `from-wei`, `to-hex`).

**Week 3 — cast (signing/tx).** `wallet sign`, `mktx`, `send`, `estimate`, `access-list`, `create2`, EIP-712 utilities, RLP en/decoders.

**Week 4 — anvil.** Tokio + axum JSON-RPC. Implement core `eth_*` and `anvil_*` RPCs against in-memory revm with mining loop. Add `--fork-url` lazy backend.

**Week 5 — forge build/test (basic).** Driver around solc binary (vendor svm-rs or shell out to `solc` directly). Discover `*.t.sol`, run setUp + test_* under revm with a minimal `Inspector`. Implement `assertEq`/asserts via cheatcode dispatcher.

**Week 6 — Cheatcodes batch 1.** Block/tx env (`warp`, `roll`, `fee`, `coinbase`), account state (`deal`, `etch`, `store`, `load`, `setNonce`), prank family (`prank`, `startPrank`, `stopPrank`), expectRevert. Generate `Vm.sol` from a Rust enum via build.rs.

**Week 7 — Cheatcodes batch 2.** Mocking (`mockCall`/`mockCallRevert`), recording (`recordLogs`, `getRecordedLogs`, state-diff), snapshots, gas metering, FS (gated), env vars.

**Week 8 — Fuzz + invariant.** Property-based fuzz using `proptest`; per-call type-aware ABI generators. Invariant: random call-sequence generator, shrinker (length-binary-search + arg minimization).

**Week 9 — Forking + script.** Fork backend with persistent-account support; multi-fork journal map. `forge script` orchestrator with `vm.broadcast`. `cast send` integration.

**Week 10 — Chisel + lint + polish.** Solidity REPL that injects user input into a template contract, recompiles, executes. forge-lint via solar AST. Documentation site (mdBook) generator. CI matrix + benchmarks.

---

## GUI (Lancer-standard pattern)

`lancer-foundry` ships as a **native GUI application** by default. The CLI surface listed in `## Granular feature catalog` is still available behind a `--cli` flag (or via the `lancer-foundry-cli` companion binary), but the **plain `lancer-foundry` invocation opens a native desktop window**, matching every other tool in the Lancer ecosystem. No terminal-only mode.

### Framework

Pure-Rust GUI stack — no JavaScript, no Node, single static binary:

| Crate | Role |
|-------|------|
| `eframe` | Cross-platform window + event loop wrapper around `egui` |
| `egui` | Immediate-mode UI widgets (panels, tables, forms, menus, drag-and-drop) |
| `egui_extras` | Sortable tables, image loaders, syntax highlighting, date pickers |
| `egui_plot` | Charts, graphs, timelines (histograms, activity meters, progress) |
| `rfd` | Native file/folder pickers (open file, save dialog) |
| `directories` | Per-OS config dir (`~/.config/lancer-foundry/`, `%APPDATA%\lancer-foundry\`, etc.) |
| `image` | Screenshot / preview rendering inside egui |
| `arboard` | Clipboard read/write for "copy result" actions |
| `muda` | Native menu bar (Linux / macOS / Windows) |
| `egui-notify` | Toast notifications |

Optional dark/light theme persisted to `lancer-foundry-config.json` via `serde_json`.

### Window layout

Three-pane layout matching the Lancer mothership GUI:

```
┌─────────────────────────────────────────────────────────────────────┐
│  File   Tools   Settings   Help                              [≡]    │
├──────────┬────────────────────────────────────────┬─────────────────┤
│          │                                        │                 │
│  Inputs  │           Main view                    │  Detail pane    │
│  / jobs  │  (table of results / source tree /     │  (full record / │
│  / scope │   findings list / packet list / etc.)  │   bytes / src / │
│          │                                        │   remediation)  │
│          │                                        │                 │
├──────────┴────────────────────────────────────────┴─────────────────┤
│  Status: idle           Items: 0          [Open]  [Run]  [Export]   │
└─────────────────────────────────────────────────────────────────────┘
```

- **Left sidebar (200-300px)** — inputs / queued jobs / scope / target list. Drag-and-drop accepted.
- **Center pane (flexible)** — primary result view appropriate to the tool (table for findings, tree for hierarchical data, code editor for source).
- **Right pane (300-400px, collapsible)** — full detail of the currently-selected item.
- **Top menu bar** — File (Open / Save / Export / Recent / Quit), Tools (per-tool actions), Settings (theme, fonts, integrations), Help (About / Docs).
- **Bottom status bar** — current activity, item counts, primary action buttons.

### Cross-tool integration with Lancer + the other 29 clones

Every Lancer-clone GUI exposes a small local HTTP server on a port auto-allocated at launch (`127.0.0.1:<port>`, written to `%TEMP%/lancer-foundry.port`):

- The main Lancer GUI discovers running tools by scanning `%TEMP%/lancer-*.port` and lists them in its sidebar.
- Right-clicking any item in any tool can "Send to <other Lancer tool>" — the receiving tool fetches the payload via its own HTTP endpoint.
- A future "Lancer Hub" tray shell can launch / focus any of the 30 GUIs from one menu.
- HTTP endpoints exposed:
  - `GET  /v1/state` — current jobs / progress
  - `POST /v1/import` — accept a target / file / record from another Lancer tool
  - `GET  /v1/export?format=json|csv|sarif|markdown` — pull the current result set in a portable format

### Async work + responsiveness

- Long-running scans run on a Tokio task pool; the GUI thread only renders.
- Progress propagates via `crossbeam::channel` from worker → UI.
- A `Stop` button cancels in-flight work via a `CancellationToken`.
- All file I/O / network I/O / subprocess work goes through async channels — the egui thread never blocks.

### Build output

The existing `build.bat` / `build-windows.bat` / `build-linux.sh` already produce a single binary. With `eframe` linked, the same binary is the GUI app.

- Windows: `build\lancer-foundry.exe` (double-click opens the GUI window).
- Linux: `build/lancer-foundry` (also installable as a `.desktop` entry via `build-linux.sh --install`).
- macOS: `build/lancer-foundry` (bundled into `lancer-foundry.app` via `cargo-bundle` in the `build-mac.sh` companion).

For CI / headless / scripting, pass `--cli` to fall through to the CLI surface from `## Granular feature catalog`.

## Build Setup (Lancer-standard pattern)

This clone follows the same build pattern as the main Lancer project — single `.exe` output via `build.bat` on Windows, `.so/elf` via `build-linux.sh` on Linux.

### Project layout

```
lancer-foundry/
├── Cargo.toml              # single-crate Rust project
├── Cargo.lock              # committed (binary, not library)
├── src/
│   └── main.rs             # entry point (scaffold)
├── build.bat               # Windows build → build/lancer-foundry.exe
├── build-windows.bat       # Windows packaged dist build → dist/lancer-foundry-windows-x86_64/
├── build-linux.sh          # Linux build → build/lancer-foundry
├── .gitignore              # plan.md (local-only) + target/ + build/ + dist/
├── README.md               # public-facing (pushed to GitHub)
└── plan.md                 # this file (gitignored — local blueprint, never pushed)
```

### Build commands

**Windows (double-click `build.bat`):**
```
build.bat
```
Produces `build\lancer-foundry.exe`. Pauses at end so the window stays open.

**Windows (packaged dist):**
```
build-windows.bat
```
Produces `dist\lancer-foundry-windows-x86_64\bin\lancer-foundry.exe` plus LICENSE/README — ready to zip.

**Linux / macOS:**
```bash
./build-linux.sh
```
Produces `build/lancer-foundry`.

### Requirements
- Rust toolchain (https://rustup.rs)
- Windows: Visual C++ Build Tools 2019/2022 (needed by some crates)

### CI

The build pattern is intentionally identical across Lancer and all 19 clones, so a single Jenkins/GitHub Actions matrix can build everything by calling `build-windows.bat` or `build-linux.sh` per repo.
