use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: mono-cli <command>");
        eprintln!("Commands:");
        eprintln!("  setup     - Enable tracking and autostart");
        eprintln!("  unsetup  - Disable tracking and autostart");
        eprintln!("  status   - Show current status");
        std::process::exit(1);
    }
    
    match args[1].as_str() {
        "setup" => {
            println!("Setting up Mono...");
            if let Err(e) = mono::tui::consent::set_consent(true) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            println!("Setup complete. Tracking enabled.");
        }
        "unsetup" => {
            println!("Removing Mono autostart...");
            if let Err(e) = mono::tui::consent::remove_autostart() {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            if let Err(e) = mono::tui::consent::set_consent(false) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
            println!("Unsetup complete. Tracking disabled.");
        }
        "status" => {
            let has_consent = mono::tui::consent::has_consent();
            let daemon_running = mono::tui::consent::is_daemon_running();
            println!("Consent: {}", if has_consent { "Enabled" } else { "Disabled" });
            println!("Daemon: {}", if daemon_running { "Running" } else { "Not running" });
        }
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            std::process::exit(1);
        }
    }
}