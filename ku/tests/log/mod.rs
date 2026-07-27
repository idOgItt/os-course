#![deny(warnings)]


use spin::Lazy;

use tracing_core::LevelFilter;
use tracing_subscriber::{self, fmt, EnvFilter};


pub fn init() {
    *INIT;
}


static INIT: Lazy<()> = Lazy::new(|| {
    let filter = EnvFilter::from_default_env().add_directive(LevelFilter::DEBUG.into());

    let format = fmt::format()
        .with_level(true)
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(true)
        .compact();

    tracing_subscriber::fmt()
        .with_ansi(false)
        .event_format(format)
        .with_env_filter(filter)
        .init();
});
