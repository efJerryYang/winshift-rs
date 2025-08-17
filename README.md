# winshift

[![Crates.io](https://img.shields.io/crates/v/winshift)](https://crates.io/crates/winshift)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A cross-platform library for monitoring window focus changes.

## Features

- Native window focus tracking on Linux via X11 (reference implementation)
- macOS support via Accessibility API (requires app tracking workaround)
- Event-driven callback system
- Minimal overhead window monitoring
- Thread-safe design

Note: The app tracking functionality on macOS exists solely to work around
platform limitations for reliable window focus detection.

## Supported Platforms

- **Linux**: Native window focus tracking via X11 (reference implementation)
- **macOS**: Window tracking via Accessibility API workaround (requires app tracking)
- **Windows**: Planned (not yet implemented)

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
winshift = "0.0.1"
```

Then see the [Examples](#examples) section for usage patterns.

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
    
    // On macOS, you can stop with:
    // winshift::stop_hook();
    
    Ok(())
}
```

**Important Notes**:

1. Handler must be thread-safe (use Arc/RwLock if needed)
2. On macOS, call `stop_hook()` to clean up
3. See [examples/](examples/) for complete implementations
4. Linux uses same API but doesn't require combined tracking

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
