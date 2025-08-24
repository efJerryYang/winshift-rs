use std::sync::{Arc, RwLock};
use winshift::{FocusChangeHandler, WindowFocusHook};

struct WindowOnlyHandler {
    current_window: Arc<RwLock<String>>,
}

impl WindowOnlyHandler {
    fn new() -> Self {
        Self {
            current_window: Arc::new(RwLock::new(String::new())),
        }
    }
}

impl FocusChangeHandler for WindowOnlyHandler {
    fn on_app_change(&self, _pid: i32, _app_name: String) {
        /* NO-OP */
    }

    fn on_window_change(&self, window_title: String) {
        let mut current_window = self.current_window.write().unwrap();

        if window_title.is_empty() {
            println!("Warning: Received empty window title");
            return;
        }

        if *current_window != window_title {
            if current_window.is_empty() {
                println!("Initial window: '{window_title}'");
            } else {
                println!(
                    "Window changed: '{}' -> '{}'",
                    *current_window, window_title
                );
            }

            *current_window = window_title;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger - set RUST_LOG=debug to see library logs
    winshift::env_logger::init();

    println!("This monitor detects only window changes.");
    println!("Application switches are ignored.");
    println!();
    println!("Try switching between:");
    println!("  - Different windows within the same app (VSCode tabs, browser tabs, etc.)");
    println!("  - Different windows across apps");
    println!();
    println!("Press Ctrl+C to exit");
    println!();

    let handler = WindowOnlyHandler::new();
    let hook = WindowFocusHook::window_only(handler);

    // Set up Ctrl+C handler with proper CFRunLoop termination
    let hook_ref = Arc::new(RwLock::new(Some(hook)));
    let hook_clone = hook_ref.clone();

    ::ctrlc::set_handler(move || {
        println!("\nShutting down window-only monitor...");
        eprintln!("[SIGNAL] Ctrl+C signal received!");

        // Signal CFRunLoop to stop
        winshift::stop_hook().expect("Failed to stop hook");

        eprintln!("[SIGNAL] Stop signal sent, exiting signal handler");
    })?;

    // Run the monitor on main thread (required for NSWorkspace and Accessibility API)
    if let Ok(guard) = hook_ref.read() {
        if let Some(ref hook) = *guard {
            if let Err(e) = hook.run() {
                eprintln!("Error running hook: {e}");
            }
        }
    }

    // When hook.run() exits, clean up
    println!("Window-only monitor stopped.");
    if let Ok(mut guard) = hook_clone.write() {
        guard.take(); // Take ownership to clean up
    }

    println!("Monitor stopped.");
    Ok(())
}
