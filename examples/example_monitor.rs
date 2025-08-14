//! Example monitor that detects both app switching and window changes
//! This example shows how the main winshift implementation handles both event types seamlessly

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use winshift::{FocusChangeHandler, WindowFocusHook};

struct ExampleHandler {
    current_app_pid: Arc<RwLock<i32>>,
    current_window: Arc<RwLock<String>>,
    last_change: Arc<RwLock<Instant>>,
    app_tracking: Arc<RwLock<HashMap<i32, (String, u32)>>>, // PID -> (App name, activation count)
}

impl ExampleHandler {
    fn new() -> Self {
        Self {
            current_app_pid: Arc::new(RwLock::new(0)),
            current_window: Arc::new(RwLock::new(String::new())),
            last_change: Arc::new(RwLock::new(Instant::now())),
            app_tracking: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl FocusChangeHandler for ExampleHandler {
    fn on_app_change(&self, pid: i32, app_name: String) {
        let mut current_pid = self.current_app_pid.write().unwrap();
        let mut last_change = self.last_change.write().unwrap();
        let now = Instant::now();

        *last_change = now;

        if *current_pid != pid {
            if *current_pid == 0 {
                println!("Initial app: {app_name} (PID: {pid})");
            } else {
                println!(
                    "Application switched: PID {} -> {} (PID: {})",
                    *current_pid, app_name, pid
                );

                let mut tracking = self.app_tracking.write().unwrap();
                let (_, count) = tracking.entry(pid).or_insert((app_name.clone(), 0));
                *count += 1;
                println!("   {app_name} has been activated {count} time(s)");
            }

            *current_pid = pid;
        }
    }

    fn on_window_change(&self, window_title: String) {
        let mut current = self.current_window.write().unwrap();
        let mut last_change = self.last_change.write().unwrap();
        let now = Instant::now();

        *last_change = now;

        if window_title.is_empty() {
            println!("Warning: Received empty window title");
            return;
        }

        if *current != window_title {
            if current.is_empty() {
                println!("Initial window: '{window_title}'");
            } else {
                println!("Window changed: '{current}' -> '{window_title}'");
            }

            *current = window_title;
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger - set RUST_LOG=debug to see library logs
    winshift::env_logger::init();

    println!("This monitor detects both application switches and window changes.");
    println!();
    println!("Try switching between:");
    println!("  - Different applications (Safari, Terminal, VSCode, etc.)");
    println!("  - Different windows within the same app");
    println!();
    println!("Press Ctrl+C to exit");
    println!();

    let handler = ExampleHandler::new();
    let hook = WindowFocusHook::new(handler);

    // Set up Ctrl+C handler with proper CFRunLoop termination
    let hook_ref = Arc::new(RwLock::new(Some(hook)));
    let hook_clone = hook_ref.clone();

    ::ctrlc::set_handler(move || {
        println!("\nShutting down monitor...");
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
    println!("Monitor stopped.");
    if let Ok(mut guard) = hook_clone.write() {
        guard.take(); // Take ownership to clean up
    }

    println!("Monitor stopped.");
    Ok(())
}
