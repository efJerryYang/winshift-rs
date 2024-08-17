use ctrlc;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use winshift::{FocusChangeHandler, WindowFocusHook};
mod logger;

struct WindowChangeHandler {
    current_window: Arc<RwLock<String>>,
    last_change: Arc<RwLock<Instant>>,
}

impl FocusChangeHandler for WindowChangeHandler {
    fn on_focus_change(&self, window_title: String) {
        let mut current = self.current_window.write().unwrap();
        let mut last_change = self.last_change.write().unwrap();
        let now = Instant::now();

        *last_change = now;

        if window_title.is_empty() {
            log_warn!("Received empty window title");
        } else if *current != window_title {
            log_info!("Window changed: '{}' -> '{}'", current, window_title);
            *current = window_title;
        } else {
            log_debug!("Window title unchanged: {}", window_title);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    logger::init();

    log_info!("Starting window focus monitoring...");

    let handler = WindowChangeHandler {
        current_window: Arc::new(RwLock::new(String::new())),
        last_change: Arc::new(RwLock::new(Instant::now())),
    };

    let hook = Arc::new(WindowFocusHook::new(handler));
    let hook_clone = hook.clone();
    ctrlc::set_handler(move || {
        println!("\nExiting...");
        if let Err(e) = hook_clone.stop() {
            log_error!("Error stopping hook: {}", e);
        }
    })
    .expect("Error setting Ctrl-C handler");
    if let Err(e) = hook.run() {
        log_error!("Error running hook: {}", e);
    }

    Ok(())
}
