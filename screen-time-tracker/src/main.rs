use log::info;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use screen_time_tracker::ipc_server;
use screen_time_tracker::session_manager::SessionManager;
use screen_time_tracker::storage::Storage;
use screen_time_tracker::tracker::Tracker;

static RUNNING: AtomicBool = AtomicBool::new(true);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();

    info!("Screen Time Tracker v0.2.0 starting...");

    // Initialize storage (runs migrations + crash recovery)
    let storage = Arc::new(Storage::new());
    info!("Database initialized");

    // Initialize X11 tracker
    let tracker = match Tracker::new() {
        Some(t) => t,
        None => {
            eprintln!("ERROR: Cannot connect to X11 display. Is X11 running?");
            eprintln!("Wayland-only sessions are not yet supported.");
            std::process::exit(1);
        }
    };
    info!("X11 tracker initialized");

    // Set up graceful shutdown via Ctrl+C / SIGTERM
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    ctrlc::set_handler(move || {
        info!("Received shutdown signal");
        running_clone.store(false, Ordering::SeqCst);
        RUNNING.store(false, Ordering::SeqCst);
    })
    .expect("Failed to set Ctrl+C handler");

    // Start the IPC HTTP server in a background thread
    let storage_for_server = storage.clone();
    let server_handle = thread::spawn(move || {
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

    // Run the session manager in the main thread
    let mut session_mgr = SessionManager::new(storage.clone(), tracker);
    info!("Session manager started — tracking active window");

    while running.load(Ordering::SeqCst) {
        session_mgr.tick();
        thread::sleep(Duration::from_secs(1));
    }

    // Graceful shutdown
    info!("Shutting down...");
    session_mgr.shutdown();
    info!("Goodbye!");
}