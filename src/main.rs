use std::sync::{Arc, Mutex};
use std::time::Instant;
use winshift::{FocusChangeHandler, WindowFocusHook};

mod logger;

struct WindowChangeHandler {
    current_window: Arc<Mutex<String>>,
    last_change: Arc<Mutex<Instant>>,
}

impl FocusChangeHandler for WindowChangeHandler {
    fn on_focus_change(&self, window_title: String) {
        let mut current = self.current_window.lock().unwrap();
        let mut last_change = self.last_change.lock().unwrap();
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
        current_window: Arc::new(Mutex::new(String::new())),
        last_change: Arc::new(Mutex::new(Instant::now())),
    };

    let hook = WindowFocusHook::new(handler);

    hook.run()?;

    Ok(())
}
