use std::sync::{Arc, RwLock};
use winshift::{FocusChangeHandler, WindowFocusHook};

struct AppOnlyHandler {
    current_app: Arc<RwLock<String>>,
    current_pid: Arc<RwLock<i32>>,
}

impl AppOnlyHandler {
    fn new() -> Self {
        Self {
            current_app: Arc::new(RwLock::new(String::new())),
            current_pid: Arc::new(RwLock::new(0)),
        }
    }
}

impl FocusChangeHandler for AppOnlyHandler {
    fn on_app_change(&self, pid: i32, app_name: String) {
        let mut current_app = self.current_app.write().unwrap();
        let mut current_pid = self.current_pid.write().unwrap();

        if *current_pid != pid {
            if current_app.is_empty() {
                println!("Initial app: {app_name} (PID: {pid})");
            } else {
                println!(
                    "Application switched: {} (PID: {}) -> {} (PID: {})",
                    *current_app, *current_pid, app_name, pid
                );
            }

            *current_app = app_name;
            *current_pid = pid;
        }
    }

    fn on_window_change(&self, _window_title: String) {
        /* NO-OP */
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger - set RUST_LOG=debug to see library logs
    winshift::env_logger::init();

    println!("This monitor detects only application switches.");
    println!("Window changes within the same application are ignored.");
    println!();
    println!("Try switching between different applications:");
    println!("  - Safari, Terminal, VSCode, Finder, etc.");
    println!();
    println!("Press Ctrl+C to exit");
    println!();

    let handler = AppOnlyHandler::new();
    let hook = Arc::new(WindowFocusHook::app_only(handler));
    let stop_handle = hook.clone();

    ::ctrlc::set_handler(move || {
        println!("\nShutting down app-only monitor...");
        eprintln!("[SIGNAL] Ctrl+C signal received!");

        if let Err(err) = stop_handle.stop() {
            eprintln!("Failed to stop hook: {err}");
        }

        eprintln!("[SIGNAL] Stop signal sent, exiting signal handler");
    })?;

    // Run the monitor on main thread (required for NSWorkspace and Accessibility API)
    if let Err(e) = hook.run() {
        eprintln!("Error running hook: {e}");
    }

    println!("App-only monitor stopped.");
    Ok(())
}
