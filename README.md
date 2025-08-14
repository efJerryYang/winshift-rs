# winshift-rs

A cross-platform Rust library for monitoring window focus changes.

## Features

- ✅ **Cross-platform**: Supports macOS, Linux (X11)
- ✅ **Event-driven**: Uses native OS event systems instead of polling
- ✅ **Thread-safe**: Safe to use across multiple threads

## Platform Implementation

| Platform | Implementation | Status |
|----------|----------------|--------|
| **macOS** | NSWorkspace + Accessibility API | ✅ Complete |
| **Linux** | X11 PropertyNotify events | ✅ Complete |

### macOS Implementation Details

- Uses NSWorkspace notifications for app switching
- Uses Accessibility API observers for window monitoring within apps
- macOS Accessibility API has no native system-wide window monitoring
- Different modes work around these API limitations
- Requires accessibility permissions

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
winshift = "0.0.1"
```

### Basic Usage

```rust
use winshift::{FocusChangeHandler, WindowFocusHook};

struct MyHandler;

impl FocusChangeHandler for MyHandler {
    fn on_app_change(&self, pid: i32, app_name: String) {
        println!("App switched: {} (PID: {})", app_name, pid);
    }

    fn on_window_change(&self, window_title: String) {
        println!("Window changed: {}", window_title);
    }
}

fn main() -> Result<(), winshift::WinshiftError> {
    let handler = MyHandler;
    let hook = WindowFocusHook::new(handler);
    hook.run() // Blocks until stop() is called
}
```

For usage examples with signal handling, logging, and error handling, see the `examples/` directory.

## Running Examples

```bash
# Basic monitor
cargo run --example example_monitor

# With debug logging
RUST_LOG=debug cargo run --example example_monitor
```

## Platform-Specific Setup

### macOS

1. **Accessibility Permissions Required**
   ```
   System Preferences > Security & Privacy > Privacy > Accessibility
   ```
   Add your application or terminal to the allowed list.


### Linux (X11)

- Works out of the box on X11 systems

## Architecture

### Event-Driven Design

Winshift-rs uses native OS event systems:

- **macOS**: NSWorkspace notifications + AX observers
- **Linux**: X11 PropertyNotify events


### Error Handling

```rust
use winshift::WinshiftError;

match hook.run() {
    Ok(()) => println!("Hook stopped normally"),
    Err(WinshiftError::PlatformError(msg)) => eprintln!("Platform error: {}", msg),
    Err(WinshiftError::InitializationError) => eprintln!("Failed to initialize"),
    Err(WinshiftError::StopError) => eprintln!("Failed to stop cleanly"),
}
```

## Implementation Notes

- Thread-safe handler access
- Proper observer lifecycle management on macOS

## License

MIT License - see [LICENSE](LICENSE) file for details.