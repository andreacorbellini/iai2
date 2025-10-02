mod client;
mod parser;
mod runner;
mod stats;

pub(crate) use client::running_on_valgrind;
pub(crate) use client::start_instrumentation;
pub(crate) use client::stop_instrumentation;
pub(crate) use parser::parse_cachegrind_output;
pub(crate) use runner::Cachegrind;
pub(crate) use stats::CachegrindStats;
