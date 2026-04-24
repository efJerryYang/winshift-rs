#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macos_memory_diagnostics is intended for macOS only");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
mod macos_repro {
    use clap::{Parser, ValueEnum};
    use std::hint::black_box;
    use std::process;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    use winshift::{
        ActiveWindowInfo, FocusChangeHandler, MonitoringMode, WindowFocusHook, WindowHookConfig,
        get_active_window_info,
    };

    #[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
    enum Mode {
        HookCycle,
        QueryLoop,
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
    enum Monitor {
        Combined,
        AppOnly,
        WindowOnly,
    }

    impl Monitor {
        fn into_winshift(self) -> MonitoringMode {
            match self {
                Self::Combined => MonitoringMode::Combined,
                Self::AppOnly => MonitoringMode::AppOnly,
                Self::WindowOnly => MonitoringMode::WindowOnly,
            }
        }
    }

    #[derive(Parser, Debug)]
    #[command(name = "macos_memory_diagnostics")]
    #[command(about = "Manual macOS memory regression diagnostics for winshift")]
    struct Args {
        #[arg(long, value_enum, default_value_t = Mode::HookCycle)]
        mode: Mode,

        #[arg(long, default_value_t = 200)]
        iterations: u64,

        #[arg(long, default_value_t = 10)]
        sample_every: u64,

        #[arg(long, value_enum, default_value_t = Monitor::Combined)]
        monitor: Monitor,

        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        embed_active_info: bool,

        #[arg(long, default_value_t = 250)]
        hook_run_ms: u64,

        #[arg(long, default_value_t = 50)]
        cooldown_ms: u64,

        #[arg(long, default_value_t = 10)]
        query_sleep_ms: u64,
    }

    #[derive(Default)]
    struct CallbackStats {
        app_changes: AtomicU64,
        window_changes: AtomicU64,
        app_info_changes: AtomicU64,
        window_info_changes: AtomicU64,
    }

    struct CountingHandler {
        stats: Arc<CallbackStats>,
    }

    impl CountingHandler {
        fn new(stats: Arc<CallbackStats>) -> Self {
            Self { stats }
        }
    }

    impl FocusChangeHandler for CountingHandler {
        fn on_app_change(&self, _pid: i32, _app_name: String) {
            self.stats.app_changes.fetch_add(1, Ordering::Relaxed);
        }

        fn on_window_change(&self, _window_title: String) {
            self.stats.window_changes.fetch_add(1, Ordering::Relaxed);
        }

        fn on_app_change_info(&self, _info: ActiveWindowInfo) {
            self.stats.app_info_changes.fetch_add(1, Ordering::Relaxed);
        }

        fn on_window_change_info(&self, _info: ActiveWindowInfo) {
            self.stats
                .window_info_changes
                .fetch_add(1, Ordering::Relaxed);
        }
    }

    struct RunStats {
        started_at: Instant,
        baseline_rss_bytes: Option<u64>,
        query_ok: u64,
        query_err: u64,
        hook_ok: u64,
        hook_err: u64,
        stop_attempts: u64,
        stop_retries: u64,
    }

    impl RunStats {
        fn new() -> Self {
            Self {
                started_at: Instant::now(),
                baseline_rss_bytes: current_rss_bytes(),
                query_ok: 0,
                query_err: 0,
                hook_ok: 0,
                hook_err: 0,
                stop_attempts: 0,
                stop_retries: 0,
            }
        }
    }

    struct StopReport {
        attempts: u64,
        retries: u64,
    }

    pub fn main() {
        let args = Args::parse();
        println!(
            "starting mode={:?} pid={} monitor={:?} embed_active_info={} iterations={} sample_every={}",
            args.mode,
            process::id(),
            args.monitor,
            args.embed_active_info,
            args.iterations,
            args.sample_every
        );
        println!(
            "tip: compare before/after fix with the same command and watch rss_mb / rss_delta_mb"
        );

        let stats = Arc::new(CallbackStats::default());
        let mut run_stats = RunStats::new();

        match args.mode {
            Mode::HookCycle => run_hook_cycles(&args, &stats, &mut run_stats),
            Mode::QueryLoop => run_query_loop(&args, &stats, &mut run_stats),
        }
    }

    fn run_hook_cycles(args: &Args, stats: &Arc<CallbackStats>, run_stats: &mut RunStats) {
        let config = WindowHookConfig {
            monitoring_mode: args.monitor.into_winshift(),
            embed_active_info: args.embed_active_info,
        };
        let mut cycle = 0_u64;

        while keep_running(cycle, args.iterations) {
            cycle += 1;
            let hook = Arc::new(WindowFocusHook::with_config(
                CountingHandler::new(Arc::clone(stats)),
                config.clone(),
            ));
            let run_done = Arc::new(AtomicBool::new(false));
            let stopper = spawn_stopper(
                Arc::clone(&hook),
                Arc::clone(&run_done),
                Duration::from_millis(args.hook_run_ms),
            );

            let run_result = hook.run();
            run_done.store(true, Ordering::Relaxed);
            let stop_report = stopper.join().unwrap_or(StopReport {
                attempts: 0,
                retries: 0,
            });

            run_stats.stop_attempts = run_stats.stop_attempts.saturating_add(stop_report.attempts);
            run_stats.stop_retries = run_stats.stop_retries.saturating_add(stop_report.retries);

            match run_result {
                Ok(()) => run_stats.hook_ok = run_stats.hook_ok.saturating_add(1),
                Err(err) => {
                    run_stats.hook_err = run_stats.hook_err.saturating_add(1);
                    eprintln!("hook_cycle_error cycle={} error={}", cycle, err);
                    print_sample("hook-cycle", cycle, stats, run_stats);
                    break;
                }
            }

            if should_sample(cycle, args.sample_every) {
                print_sample("hook-cycle", cycle, stats, run_stats);
            }

            if args.cooldown_ms > 0 {
                thread::sleep(Duration::from_millis(args.cooldown_ms));
            }
        }

        print_sample("hook-cycle", cycle, stats, run_stats);
    }

    fn run_query_loop(args: &Args, stats: &Arc<CallbackStats>, run_stats: &mut RunStats) {
        let mut iteration = 0_u64;

        while keep_running(iteration, args.iterations) {
            iteration += 1;
            match get_active_window_info() {
                Ok(info) => {
                    run_stats.query_ok = run_stats.query_ok.saturating_add(1);
                    black_box((
                        info.process_id,
                        info.window_id,
                        info.title.len(),
                        info.app_name.len(),
                    ));
                }
                Err(err) => {
                    run_stats.query_err = run_stats.query_err.saturating_add(1);
                    if should_sample(iteration, args.sample_every) {
                        eprintln!("query_loop_error iteration={} error={}", iteration, err);
                    }
                }
            }

            if should_sample(iteration, args.sample_every) {
                print_sample("query-loop", iteration, stats, run_stats);
            }

            if args.query_sleep_ms > 0 {
                thread::sleep(Duration::from_millis(args.query_sleep_ms));
            }
        }

        print_sample("query-loop", iteration, stats, run_stats);
    }

    fn spawn_stopper(
        hook: Arc<WindowFocusHook>,
        run_done: Arc<AtomicBool>,
        run_for: Duration,
    ) -> thread::JoinHandle<StopReport> {
        thread::spawn(move || {
            thread::sleep(run_for);
            let mut attempts = 0_u64;
            let mut retries = 0_u64;

            loop {
                if run_done.load(Ordering::Relaxed) {
                    return StopReport { attempts, retries };
                }

                attempts = attempts.saturating_add(1);
                match hook.stop() {
                    Ok(()) => return StopReport { attempts, retries },
                    Err(_) => {
                        retries = retries.saturating_add(1);
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            }
        })
    }

    fn keep_running(iteration: u64, limit: u64) -> bool {
        limit == 0 || iteration < limit
    }

    fn should_sample(iteration: u64, sample_every: u64) -> bool {
        sample_every > 0 && iteration % sample_every == 0
    }

    fn current_rss_bytes() -> Option<u64> {
        let pid = Pid::from(process::id() as usize);
        let mut system = System::new();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            false,
            ProcessRefreshKind::nothing().with_memory(),
        );
        system.process(pid).map(|process| process.memory())
    }

    fn print_sample(
        label: &str,
        iteration: u64,
        stats: &Arc<CallbackStats>,
        run_stats: &RunStats,
    ) {
        let rss_bytes = current_rss_bytes();
        let rss_mb = rss_bytes.map(bytes_to_mb).unwrap_or(-1.0);
        let rss_delta_mb = match (run_stats.baseline_rss_bytes, rss_bytes) {
            (Some(base), Some(now)) => bytes_to_mb(now.saturating_sub(base)),
            _ => -1.0,
        };
        let elapsed = run_stats.started_at.elapsed().as_secs_f64();

        println!(
            "sample label={} iteration={} elapsed_s={:.3} rss_mb={:.3} rss_delta_mb={:.3} query_ok={} query_err={} hook_ok={} hook_err={} stop_attempts={} stop_retries={} app_changes={} window_changes={} app_info={} window_info={}",
            label,
            iteration,
            elapsed,
            rss_mb,
            rss_delta_mb,
            run_stats.query_ok,
            run_stats.query_err,
            run_stats.hook_ok,
            run_stats.hook_err,
            run_stats.stop_attempts,
            run_stats.stop_retries,
            stats.app_changes.load(Ordering::Relaxed),
            stats.window_changes.load(Ordering::Relaxed),
            stats.app_info_changes.load(Ordering::Relaxed),
            stats.window_info_changes.load(Ordering::Relaxed),
        );
    }

    fn bytes_to_mb(bytes: u64) -> f64 {
        bytes as f64 / (1024.0 * 1024.0)
    }
}

#[cfg(target_os = "macos")]
fn main() {
    macos_repro::main();
}
