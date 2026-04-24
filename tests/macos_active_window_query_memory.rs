#![cfg(target_os = "macos")]

use std::hint::black_box;
use std::thread;
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
use winshift::get_active_window_info;

fn current_rss_bytes() -> Option<u64> {
    let pid = Pid::from(std::process::id() as usize);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        false,
        ProcessRefreshKind::nothing().with_memory(),
    );
    system.process(pid).map(|process| process.memory())
}

#[test]
#[ignore = "manual macOS regression test; requires foreground GUI session and Accessibility permission"]
fn query_loop_memory_stays_bounded() {
    let baseline = current_rss_bytes().expect("baseline RSS should be available");
    let iterations = 5_000_u64;
    let max_delta_bytes = 8_u64 * 1024 * 1024;
    let mut ok = 0_u64;

    for iteration in 0..iterations {
        let info = get_active_window_info().unwrap_or_else(|err| {
            panic!(
                "get_active_window_info failed at iteration {}: {}",
                iteration + 1,
                err
            )
        });
        ok = ok.saturating_add(1);
        black_box((
            info.process_id,
            info.window_id,
            info.title.len(),
            info.app_name.len(),
        ));
        thread::sleep(Duration::from_millis(1));
    }

    let end = current_rss_bytes().expect("ending RSS should be available");
    let delta = end.saturating_sub(baseline);

    assert_eq!(ok, iterations, "all query iterations should succeed");
    assert!(
        delta <= max_delta_bytes,
        "query loop RSS delta too large: baseline={} end={} delta={} bytes",
        baseline,
        end,
        delta
    );
}
