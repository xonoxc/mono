use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use mono::autostart;
use mono::ipc_server;
use mono::session_manager::SessionManager;
use mono::storage::Storage;
use mono::window_manager::{self};

static RUNNING: AtomicBool = AtomicBool::new(true);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    info!("Screen Time Tracker v0.2.0 starting...");

    let storage = Arc::new(Storage::new());
    info!("Database initialized");

    let tracker = match window_manager::create_manager() {
        Some(t) => t,
        None => {
            eprintln!("ERROR: Failed to initialize window manager.");
            eprintln!("Please ensure you're running X11 or a supported Wayland compositor (Hyprland, Sway, GNOME, KDE).");
            std::process::exit(1);
        }
    };
    info!("Window manager initialized ({})", tracker.name());

    if !autostart::is_autostart_enabled() {
        match autostart::setup_autostart() {
            Ok(_) => info!("Autostart configured successfully"),
            Err(e) => warn!("Failed to configure autostart: {}", e),
        }
    } else {
        info!("Autostart already enabled");
    }

    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    ctrlc::set_handler(move || {
        info!("Received shutdown signal");
        running_clone.store(false, Ordering::SeqCst);
        RUNNING.store(false, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    let storage_for_server = storage.clone();
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        rt.block_on(async {
            if let Err(e) = ipc_server::start_server(storage_for_server).await {
                eprintln!("IPC server error: {}", e);
            }
        });
    });

    let mut session_mgr = SessionManager::new(storage.clone(), tracker);
    info!("Session manager started — tracking active window");

    while running.load(Ordering::SeqCst) {
        session_mgr.tick();
        thread::sleep(Duration::from_secs(1));
    }

    info!("Shutting down...");
    session_mgr.shutdown();
    info!("Goodbye!");
}
