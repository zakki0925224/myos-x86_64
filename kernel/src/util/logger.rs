use crate::{graphics::frame_buf_console, print, theme::GLOBAL_THEME, util};

static mut LOGGER: SimpleLogger = SimpleLogger::new(LogLevel::max());

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    const fn max() -> Self {
        LogLevel::Trace
    }

    fn to_str(self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => " WARN",
            LogLevel::Info => " INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }
}

struct SimpleLogger {
    max_level: LogLevel,
}

impl SimpleLogger {
    const fn new(max_level: LogLevel) -> Self {
        Self { max_level }
    }

    fn enabled(&self, level: LogLevel) -> bool {
        level <= self.max_level
    }

    fn log(&self, level: LogLevel, args: core::fmt::Arguments, file: &str, line: u32, col: u32) {
        if !self.enabled(level) {
            return;
        }

        let fore_color = match level {
            LogLevel::Error => GLOBAL_THEME.log_color_error,
            LogLevel::Warn => GLOBAL_THEME.log_color_warn,
            LogLevel::Info => GLOBAL_THEME.log_color_info,
            LogLevel::Debug => GLOBAL_THEME.log_color_debug,
            LogLevel::Trace => GLOBAL_THEME.log_color_trace,
        };

        let _ = frame_buf_console::set_fore_color(fore_color);

        let uptime = util::time::global_uptime();
        if uptime.is_zero() {
            print!("[??????.???]");
        } else {
            let ms = uptime.as_millis() as usize;
            print!("[{:06}.{:03}]", ms / 1000, ms % 1000);
        }

        print!("[{}]: ", level.to_str());

        if level == LogLevel::Error {
            print!("{}@{}:{}: ", file, line, col);
        }

        print!("{:?}\n", args);

        let _ = frame_buf_console::reset_fore_color();
    }
}

pub unsafe fn log(level: LogLevel, args: core::fmt::Arguments, file: &str, line: u32, col: u32) {
    LOGGER.log(level, args, file, line, col);
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        unsafe {
            $crate::util::logger::log(
                $crate::util::logger::LogLevel::Info,
                format_args!($($arg)*),
                file!(),
                line!(),
                column!()
            );
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        unsafe {
            $crate::util::logger::log(
                $crate::util::logger::LogLevel::Debug,
                format_args!($($arg)*),
                file!(),
                line!(),
                column!()
            );
        }
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        unsafe {
            $crate::util::logger::log(
                $crate::util::logger::LogLevel::Warn,
                format_args!($($arg)*),
                file!(),
                line!(),
                column!()
            );
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        unsafe {
            $crate::util::logger::log(
                $crate::util::logger::LogLevel::Error,
                format_args!($($arg)*),
                file!(),
                line!(),
                column!()
            );
        }
    };
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => {
        unsafe {
            $crate::util::logger::log(
                $crate::util::logger::LogLevel::Trace,
                format_args!($($arg)*),
                file!(),
                line!(),
                column!()
            );
        }
    };
}
