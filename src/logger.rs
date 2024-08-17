use chrono::Local;
use colored::*;
use log::{LevelFilter, Record};
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use std::sync::Once;

static INIT: Once = Once::new();

pub fn init() {
    INIT.call_once(|| {
        let stdout = ConsoleAppender::builder()
            .encoder(Box::new(PatternEncoder::new(
                "{d(%Y-%m-%d %H:%M:%S)(local)} [{h({l})}] {m}{n}",
            )))
            .build();

        let config = Config::builder()
            .appender(Appender::builder().build("stdout", Box::new(stdout)))
            .build(Root::builder().appender("stdout").build(LevelFilter::Trace))
            .unwrap();

        log4rs::init_config(config).unwrap();
    });
}

pub fn log(record: &Record) {
    let level = match record.level() {
        log::Level::Error => record.level().to_string().red(),
        log::Level::Warn => record.level().to_string().yellow(),
        log::Level::Info => record.level().to_string().green(),
        log::Level::Debug => record.level().to_string().blue(),
        log::Level::Trace => record.level().to_string().magenta(),
    };

    let timestamp = Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
        .bright_black();

    let file = record.file().unwrap_or("unknown");
    let line = record.line().unwrap_or(0);

    println!(
        "{} [{}] {}:{} - {}",
        timestamp,
        level,
        file,
        line,
        record.args()
    );
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::log(&log::Record::builder()
            .args(format_args!($($arg)*))
            .level(log::Level::Info)
            .target(module_path!())
            .file(Some(file!()))
            .line(Some(line!()))
            .build()
        );
    };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::logger::log(&log::Record::builder()
            .args(format_args!($($arg)*))
            .level(log::Level::Error)
            .target(module_path!())
            .file(Some(file!()))
            .line(Some(line!()))
            .build()
        );
    };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::logger::log(&log::Record::builder()
            .args(format_args!($($arg)*))
            .level(log::Level::Warn)
            .target(module_path!())
            .file(Some(file!()))
            .line(Some(line!()))
            .build()
        );
    };
}

#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::logger::log(&log::Record::builder()
            .args(format_args!($($arg)*))
            .level(log::Level::Debug)
            .target(module_path!())
            .file(Some(file!()))
            .line(Some(line!()))
            .build()
        );
    };
}

#[macro_export]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        $crate::logger::log(&log::Record::builder()
            .args(format_args!($($arg)*))
            .level(log::Level::Trace)
            .target(module_path!())
            .file(Some(file!()))
            .line(Some(line!()))
            .build()
        );
    };
}