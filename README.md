# winshift

[![Crates.io](https://img.shields.io/crates/v/winshift)](https://crates.io/crates/winshift)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A cross-platform library for monitoring window focus changes.

## Features

- Native window focus tracking on Linux via X11 (reference implementation)
- macOS support via Accessibility API (requires app tracking workaround)
- Event-driven callback system
- Minimal overhead window monitoring
- Optional embedded active window info in callbacks (macOS)

Note: The app tracking functionality on macOS exists solely to work around
platform limitations for reliable window focus detection.

## Supported Platforms

- **Linux**: Native window focus tracking via X11 (reference implementation)
- **macOS**: Window tracking via Accessibility API workaround (requires app tracking)
- **Windows**: Planned (not yet implemented)

## Quick Start

```sh
cargo add winshift
```

See the [Examples](#examples) section for usage patterns.

## Examples

### Illustrative Usage Pattern

This simplified example shows the basic structure. For complete, working examples see the [examples/](examples/) directory.

```rust,no_run
use winshift::{FocusChangeHandler, WindowFocusHook};
use std::sync::Arc;

struct MyHandler;

impl FocusChangeHandler for MyHandler {
    fn on_app_change(&self, pid: i32, app_name: String) {
        println!("App changed: {} (PID: {})", app_name, pid);
    }

    fn on_window_change(&self, window_title: String) {
        println!("Window changed: {}", window_title);
    }
}

fn main() -> Result<(), winshift::WinshiftError> {
    let handler = Arc::new(MyHandler);
    let hook = WindowFocusHook::new(handler);
    
    // Start monitoring (runs in current thread)
    hook.run()?;
    
    // On macOS, you can stop from another thread with:
    // hook.stop()?;
    
    Ok(())
}
```

### macOS: Embedded ActiveWindowInfo (no extra queries)

When enabled, macOS event callbacks also include the full `ActiveWindowInfo` so apps don’t need to call `get_active_window_info()` after each event. This reduces overhead and simplifies consumers like chronicle.

How to enable:

```rust
use winshift::{FocusChangeHandler, WindowFocusHook, WindowHookConfig};

struct Handler;

impl FocusChangeHandler for Handler {
    fn on_app_change(&self, pid: i32, app_name: String) {
        println!("app: {app_name} ({pid})");
    }
    fn on_window_change(&self, title: String) {
        println!("win: {title}");
    }

    // macOS-only rich payloads
    #[cfg(target_os = "macos")]
    fn on_app_change_info(&self, info: winshift::ActiveWindowInfo) {
        println!("app+info: {} pid={} exe={} wid={} bounds=({},{} {}x{})",
            info.app_name, info.process_id, info.proc_path, info.window_id,
            info.bounds.x, info.bounds.y, info.bounds.width, info.bounds.height);
    }
    #[cfg(target_os = "macos")]
    fn on_window_change_info(&self, info: winshift::ActiveWindowInfo) {
        println!("win+info: '{}' exe={}", info.title, info.proc_path);
    }
}

fn main() -> Result<(), winshift::WinshiftError> {
    winshift::env_logger::init();
    let mut cfg = WindowHookConfig::default();
    cfg.embed_active_info = true; // opt-in to richer payloads
    let hook = WindowFocusHook::with_config(Handler, cfg);
    hook.run()
}
```

See `examples/embed_info_monitor.rs` for a complete example.

**Important Notes**:

1. Handler must be thread-safe (use Arc/RwLock if needed)
2. On macOS, call `WindowFocusHook::stop()` on the instance (e.g. from a signal handler)
3. See [examples/](examples/) for complete implementations
4. Linux uses same API but doesn't require combined tracking
5. Note: Implementation uses unsafe code and assumes single-threaded usage for now
6. Embedded info callbacks are currently macOS-only; other platforms still receive the base callbacks

## Platform Notes

### Linux (Reference Implementation)

- Pure window focus tracking via X11 protocol
- No app tracking required
- Tested with common desktop environments (GNOME, KDE, etc.)

### macOS Requirements

- Requires Accessibility permissions
- Enable in System Preferences > Security & Privacy > Privacy > Accessibility

### Windows

- Not yet implemented (planned for future release)

## API Documentation

Full API documentation is available via `cargo doc --open`.

## Contributing

Contributions are welcome! Please open issues or pull requests on GitHub.

## License

MIT - See [LICENSE](LICENSE) for details.
