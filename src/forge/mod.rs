pub mod compiler;
pub mod doc;
pub mod filter;
pub mod fuzz;
pub mod invariant;
pub mod lint;
pub mod reporter;
pub mod runner;
pub mod script;

use std::path::PathBuf;
use std::time::Instant;
use clap::Parser;

use filter::TestFilter;
use runner::ForgeTestRunner;
use reporter::{print_results, print_gas_report};

#[derive(Parser, Debug)]
pub struct ForgeArgs {
    #[command(subcommand)]
    pub command: ForgeCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum ForgeCommand {
    /// Build smart contracts
    Build(BuildArgs),
    /// Run tests
    Test(TestArgs),
    /// Deploy a contract or run a script
    Script(ScriptArgs),
    /// Show test coverage
    Coverage(CoverageArgs),
    /// Format Solidity source
    Fmt(FmtArgs),
    /// Lint Solidity source (security checks)
    Lint(LintArgs),
    /// Generate documentation from NatSpec
    Doc(DocArgs),
    /// Initialise a new project
    Init(InitArgs),
    /// Install dependencies
    Install(InstallArgs),
    /// Show forge version
    Version,
    /// Dump Vm.sol interface
    #[command(name = "vm-sol")]
    VmSol,
}

#[derive(Parser, Debug)]
pub struct BuildArgs {
    #[arg(long, default_value = "src")]       pub src: String,
    #[arg(long, default_value = "out")]       pub out: String,
    #[arg(long)]                               pub via_ir: bool,
    #[arg(long)]                               pub no_optimize: bool,
    #[arg(long, default_value = "200")]       pub optimizer_runs: u32,
    #[arg(long, default_value = "foundry.toml")] pub config: String,
}

#[derive(Parser, Debug)]
pub struct TestArgs {
    #[arg(long, short = 'v', action = clap::ArgAction::Count)] pub verbosity: u8,
    #[arg(long = "match-contract", short = 'C')] pub match_contract: Option<String>,
    #[arg(long = "match-test", short = 't')]     pub match_test: Option<String>,
    #[arg(long = "match-path")]                  pub match_path: Option<String>,
    #[arg(long = "no-match-contract")]           pub no_match_contract: Option<String>,
    #[arg(long = "no-match-test")]               pub no_match_test: Option<String>,
    #[arg(long, default_value = "256")]          pub fuzz_runs: u32,
    #[arg(long)]                                 pub gas_report: bool,
    #[arg(long)]                                 pub fail_fast: bool,
    #[arg(long = "root")]                        pub root: Option<PathBuf>,
    #[arg(long = "fork-url")]                    pub fork_url: Option<String>,
    #[arg(long = "fork-block-number")]           pub fork_block: Option<u64>,
}

#[derive(Parser, Debug)]
pub struct ScriptArgs {
    pub script: String,
    #[arg(long = "sig", default_value = "")] pub sig: String,
    #[arg(long = "rpc-url")]                 pub rpc_url: Option<String>,
    #[arg(long)]                             pub broadcast: bool,
    #[arg(long = "private-key")]             pub private_key: Option<String>,
    #[arg(long = "verify")]                  pub verify: bool,
    pub args: Vec<String>,
}

#[derive(Parser, Debug)]
pub struct CoverageArgs {
    #[arg(long = "report", default_value = "summary")] pub report: String,
}

#[derive(Parser, Debug)]
pub struct FmtArgs {
    #[arg(long)] pub check: bool,
    pub paths: Vec<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct LintArgs {
    #[arg(default_value = ".")] pub path: PathBuf,
    #[arg(long = "json")]       pub json: bool,
}

#[derive(Parser, Debug)]
pub struct DocArgs {
    #[arg(long, default_value = "src")] pub src: String,
    #[arg(long, default_value = "docs")] pub out: String,
}

#[derive(Parser, Debug)]
pub struct InitArgs {
    #[arg(default_value = ".")] pub root: PathBuf,
    #[arg(long, default_value = "CounterProject")] pub template: String,
}

#[derive(Parser, Debug)]
pub struct InstallArgs {
    pub deps: Vec<String>,
    #[arg(long)] pub no_commit: bool,
}

pub async fn run(args: ForgeArgs) -> eyre::Result<()> {
    match args.command {
        ForgeCommand::Build(a)    => cmd_build(a),
        ForgeCommand::Test(a)     => cmd_test(a),
        ForgeCommand::Script(a)   => cmd_script(a),
        ForgeCommand::Coverage(_) => { println!("Coverage: run with --report lcov to generate lcov.info"); Ok(()) }
        ForgeCommand::Fmt(a)      => cmd_fmt(a),
        ForgeCommand::Lint(a)     => cmd_lint(a),
        ForgeCommand::Doc(a)      => cmd_doc(a),
        ForgeCommand::Init(a)     => cmd_init(a),
        ForgeCommand::Install(a)  => cmd_install(a),
        ForgeCommand::Version     => { println!("lancer-foundry forge {}", env!("CARGO_PKG_VERSION")); Ok(()) }
        ForgeCommand::VmSol       => { print!("{}", crate::cheatcodes::vm_gen::generate_vm_sol()); Ok(()) }
    }
}

fn cmd_build(args: BuildArgs) -> eyre::Result<()> {
    let root = std::env::current_dir()?;
    let mut sources = compiler::find_sol_files(&root.join(&args.src), "**/*.sol");
    sources.extend(compiler::find_sol_files(&root.join("lib"), "**/*.sol"));

    if sources.is_empty() {
        println!("No .sol files found under {}", args.src);
        return Ok(());
    }

    println!("Compiling {} files...", sources.len());
    let start = Instant::now();
    let compiled = compiler::compile(
        &sources,
        &[root.clone()],
        &[],
        !args.no_optimize,
        args.optimizer_runs,
        args.via_ir,
    )?;

    let out_dir = root.join(&args.out);
    std::fs::create_dir_all(&out_dir)?;
    for (key, contract) in &compiled {
        let artifact = serde_json::json!({
            "contractName": &contract.name,
            "abi": &contract.abi,
            "bytecode": format!("0x{}", hex::encode(&contract.bytecode)),
            "deployedBytecode": format!("0x{}", hex::encode(&contract.deployed_bytecode)),
        });
        let out_file = out_dir.join(format!("{}.json", contract.name));
        std::fs::write(&out_file, serde_json::to_string_pretty(&artifact)?)?;
    }

    println!("Compiled {} contracts in {:.2?}", compiled.len(), start.elapsed());
    Ok(())
}

fn cmd_test(args: TestArgs) -> eyre::Result<()> {
    let root = args.root.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
    let filter = TestFilter::new(
        args.match_contract,
        args.match_test,
        args.match_path,
        args.no_match_contract,
        args.no_match_test,
    );

    let runner = ForgeTestRunner {
        root,
        filter,
        verbosity: args.verbosity,
        fuzz_runs: args.fuzz_runs,
        gas_report: args.gas_report,
        fail_fast: args.fail_fast,
    };

    let start = Instant::now();
    let results = runner.run()?;

    print_results(&results, start.elapsed(), args.verbosity);
    if args.gas_report { print_gas_report(&results); }

    let any_failed = results.iter().any(|s| s.failed() > 0 || s.setup_failed);
    if any_failed { std::process::exit(1); }
    Ok(())
}

fn cmd_script(args: ScriptArgs) -> eyre::Result<()> {
    let root = std::env::current_dir()?;
    let script = script::ForgeScript {
        root,
        rpc_url: args.rpc_url,
        broadcast: args.broadcast,
        private_key: args.private_key,
        chain_id: 1,
        verify: args.verify,
    };
    script.run(&args.script, &args.sig, &args.args)?;
    Ok(())
}

fn cmd_fmt(args: FmtArgs) -> eyre::Result<()> {
    // Basic formatter: normalize whitespace and indentation
    let paths = if args.paths.is_empty() {
        compiler::find_sol_files(&std::env::current_dir()?, "**/*.sol")
    } else {
        args.paths
    };

    let mut changed = 0;
    for path in &paths {
        if let Ok(source) = std::fs::read_to_string(path) {
            let formatted = format_solidity(&source);
            if formatted != source {
                if args.check {
                    eprintln!("Not formatted: {}", path.display());
                    changed += 1;
                } else {
                    std::fs::write(path, &formatted)?;
                    changed += 1;
                }
            }
        }
    }

    if args.check && changed > 0 {
        eyre::bail!("{changed} file(s) not formatted");
    }
    if !args.check {
        println!("Formatted {changed} file(s)");
    } else {
        println!("All files formatted correctly");
    }
    Ok(())
}

fn format_solidity(source: &str) -> String {
    // Very basic: normalize trailing whitespace and ensure single blank lines
    let lines: Vec<&str> = source.lines().collect();
    let mut out = Vec::new();
    let mut prev_blank = false;
    for line in lines {
        let trimmed_end = line.trim_end();
        let is_blank = trimmed_end.is_empty();
        if is_blank && prev_blank { continue; } // collapse double blank lines
        out.push(trimmed_end);
        prev_blank = is_blank;
    }
    out.join("\n") + "\n"
}

fn cmd_lint(args: LintArgs) -> eyre::Result<()> {
    let files = compiler::find_sol_files(&args.path, "**/*.sol");
    if files.is_empty() {
        println!("No .sol files found");
        return Ok(());
    }
    let findings = lint::lint_files(&files);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&findings)?);
    } else {
        lint::print_findings(&findings);
    }
    let high_count = findings.iter().filter(|f| f.severity == lint::Severity::High).count();
    if high_count > 0 { std::process::exit(1); }
    Ok(())
}

fn cmd_doc(args: DocArgs) -> eyre::Result<()> {
    let root = std::env::current_dir()?;
    let src_files = compiler::find_sol_files(&root.join(&args.src), "**/*.sol");
    let generator = doc::DocGenerator { root: root.clone(), out: root.join(&args.out) };
    generator.generate(&src_files)
}

fn cmd_init(args: InitArgs) -> eyre::Result<()> {
    let root = &args.root;
    std::fs::create_dir_all(root.join("src"))?;
    std::fs::create_dir_all(root.join("test"))?;
    std::fs::create_dir_all(root.join("script"))?;
    std::fs::create_dir_all(root.join("lib"))?;

    // Write Counter.sol
    std::fs::write(root.join("src/Counter.sol"), COUNTER_SOL)?;
    std::fs::write(root.join("test/Counter.t.sol"), COUNTER_TEST_SOL)?;
    std::fs::write(root.join("script/Counter.s.sol"), COUNTER_SCRIPT_SOL)?;
    std::fs::write(root.join("foundry.toml"), FOUNDRY_TOML)?;
    std::fs::write(root.join(".gitignore"), "out/\ncache/\nbroadcast/\n")?;

    println!("Initialized project at {}", root.display());
    Ok(())
}

fn cmd_install(args: InstallArgs) -> eyre::Result<()> {
    for dep in &args.deps {
        println!("Installing {dep}...");
        let output = std::process::Command::new("git")
            .args(["submodule", "add", dep, &format!("lib/{}", dep.split('/').last().unwrap_or(dep))])
            .output();
        match output {
            Ok(o) if o.status.success() => println!("  ✓ installed {dep}"),
            Ok(o) => eprintln!("  ✗ failed: {}", String::from_utf8_lossy(&o.stderr)),
            Err(e) => eprintln!("  ✗ {e}"),
        }
    }
    Ok(())
}

const COUNTER_SOL: &str = r#"// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

contract Counter {
    uint256 public number;

    function setNumber(uint256 newNumber) public {
        number = newNumber;
    }

    function increment() public {
        number++;
    }
}
"#;

const COUNTER_TEST_SOL: &str = r#"// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import {Counter} from "../src/Counter.sol";

contract CounterTest is Test {
    Counter public counter;

    function setUp() public {
        counter = new Counter();
        counter.setNumber(0);
    }

    function test_Increment() public {
        counter.increment();
        assertEq(counter.number(), 1);
    }

    function testFuzz_SetNumber(uint256 x) public {
        counter.setNumber(x);
        assertEq(counter.number(), x);
    }
}
"#;

const COUNTER_SCRIPT_SOL: &str = r#"// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script} from "forge-std/Script.sol";
import {Counter} from "../src/Counter.sol";

contract CounterScript is Script {
    Counter public counter;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();
        counter = new Counter();
        vm.stopBroadcast();
    }
}
"#;

const FOUNDRY_TOML: &str = r#"[profile.default]
src = "src"
out = "out"
libs = ["lib"]
optimizer = true
optimizer_runs = 200
"#;
