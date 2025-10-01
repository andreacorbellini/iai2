mod client;
mod runner;

pub(crate) use crate::valgrind::client::running_on_valgrind;
pub(crate) use crate::valgrind::client::start_instrumentation;
pub(crate) use crate::valgrind::client::stop_instrumentation;
pub(crate) use crate::valgrind::runner::Cachegrind;
