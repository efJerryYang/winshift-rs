//! macOS-only example showing embedded ActiveWindowInfo in callbacks

#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use winshift::{env_logger, FocusChangeHandler, WindowFocusHook, WindowHookConfig};

    env_logger::init();

    struct Handler;
    impl FocusChangeHandler for Handler {
        fn on_app_change(&self, pid: i32, app_name: String) {
            println!("[app] {app_name} (pid {pid})");
        }
        fn on_window_change(&self, title: String) {
            println!("[win] '{title}'");
        }
        fn on_app_change_info(&self, info: winshift::ActiveWindowInfo) {
            println!(
                "[app+info] pid={}, app='{}', title='{}', wid={}, bounds=({},{} {}x{}), exe={}",
                info.process_id,
                info.app_name,
                info.title,
                info.window_id,
                info.bounds.x,
                info.bounds.y,
                info.bounds.width,
                info.bounds.height,
                info.proc_path
            );
        }
        fn on_window_change_info(&self, info: winshift::ActiveWindowInfo) {
            println!(
                "[win+info] pid={}, app='{}', title='{}', wid={}, exe={}",
                info.process_id, info.app_name, info.title, info.window_id, info.proc_path
            );
        }
    }

    let handler = Handler;
    let config = WindowHookConfig {
        embed_active_info: true,
        ..Default::default()
    };

    let hook = WindowFocusHook::with_config(handler, config);

    // Allow Ctrl+C to stop
    let _ = ctrlc::set_handler(move || {
        let _ = winshift::stop_hook();
    });

    hook.run()?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This example runs on macOS only.");
}
