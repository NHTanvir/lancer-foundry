use rustyline::{DefaultEditor, error::ReadlineError};
use crate::chisel::session::ChiselSession;

const BANNER: &str = r#"
  ██████╗██╗  ██╗██╗███████╗███████╗██╗
 ██╔════╝██║  ██║██║██╔════╝██╔════╝██║
 ██║     ███████║██║███████╗█████╗  ██║
 ██║     ██╔══██║██║╚════██║██╔══╝  ██║
 ╚██████╗██║  ██║██║███████║███████╗███████╗
  ╚═════╝╚═╝  ╚═╝╚═╝╚══════╝╚══════╝╚══════╝
lancer-foundry chisel — Solidity REPL

Type Solidity expressions or statements.
Special commands: !help !quit !clear !source !traces
"#;

pub fn run_repl(fork_url: Option<String>) -> eyre::Result<()> {
    println!("{BANNER}");

    let mut session = ChiselSession::new(fork_url);
    let mut rl = DefaultEditor::new()?;

    // Load history
    let hist_file = dirs::home_dir().map(|h| h.join(".chisel_history"));
    if let Some(ref f) = hist_file {
        let _ = rl.load_history(f);
    }

    loop {
        let readline = rl.readline("➜ ");
        match readline {
            Ok(line) => {
                let line = line.trim().to_string();
                if line.is_empty() { continue; }
                rl.add_history_entry(&line).ok();

                if line.starts_with('!') {
                    handle_command(&mut session, &line);
                    continue;
                }

                match session.execute_line(&line) {
                    Ok(result) => println!("{result}"),
                    Err(e) => eprintln!("Error: {e}"),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!("Bye!");
                break;
            }
            Err(e) => {
                eprintln!("Input error: {e}");
                break;
            }
        }
    }

    if let Some(ref f) = hist_file {
        let _ = rl.save_history(f);
    }
    Ok(())
}

fn handle_command(session: &mut ChiselSession, cmd: &str) {
    match cmd {
        "!help" => print_help(),
        "!quit" | "!exit" => {
            println!("Bye!");
            std::process::exit(0);
        }
        "!clear" => {
            session.clear();
            println!("Session cleared.");
        }
        "!source" => println!("{}", session.show_source()),
        "!traces" => println!("(trace collection not yet wired to inspector output)"),
        "!list" => {
            println!("Session history:");
            for (i, h) in session.history.iter().enumerate() {
                println!("  {i}: {h}");
            }
        }
        _ if cmd.starts_with("!fork ") => {
            let url = cmd.trim_start_matches("!fork ").trim().to_string();
            session.fork_url = Some(url.clone());
            println!("Fork set to {url}");
        }
        _ => println!("Unknown command: {cmd}. Type !help for help."),
    }
}

fn print_help() {
    println!(
        r#"
Chisel commands:
  !help         Show this help
  !quit         Exit the REPL
  !clear        Clear the current session
  !source       Show the generated contract source
  !list         Show input history
  !traces       Show execution traces
  !fork <url>   Set fork URL

Solidity tips:
  - Enter any Solidity expression/statement
  - Use 'uint x = 1 + 2;' to declare variables
  - Use console.log(x) to print values (requires forge-std import)
"#
    );
}
