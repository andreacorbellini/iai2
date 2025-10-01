use crate::valgrind::stats::CachegrindStats;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::num::ParseIntError;
use std::path::Path;

pub(crate) fn parse_cachegrind_output<P: AsRef<Path>>(
    file: P,
) -> Result<CachegrindStats, ParseError> {
    let mut events_line = None;
    let mut summary_line = None;

    let file_in = File::open(file).map_err(ParseError::OpenError)?;

    for line in BufReader::new(file_in).lines() {
        let line = line.map_err(ParseError::ReadError)?;
        if let Some(line) = line.strip_prefix("events: ") {
            events_line = Some(line.trim().to_owned());
        }
        if let Some(line) = line.strip_prefix("summary: ") {
            summary_line = Some(line.trim().to_owned());
        }
    }

    let events_line = events_line.ok_or(ParseError::EventsNotFound)?;
    let summary_line = summary_line.ok_or(ParseError::SummaryNotFound)?;

    let mut events = HashMap::new();
    for (key, count) in events_line
        .split_whitespace()
        .zip(summary_line.split_whitespace())
    {
        let count = count.parse::<u64>().map_err(ParseError::InvalidNumber)?;
        events.insert(key, count);
    }

    let get = |key| events.get(key).copied().unwrap_or_default();

    Ok(CachegrindStats {
        instruction_reads: get("Ir"),
        instruction_l1_misses: get("I1mr"),
        instruction_cache_misses: get("ILmr"),
        data_reads: get("Dr"),
        data_l1_read_misses: get("D1mr"),
        data_cache_read_misses: get("DLmr"),
        data_writes: get("Dw"),
        data_l1_write_misses: get("D1mw"),
        data_cache_write_misses: get("DLmw"),
    })
}

#[derive(Debug)]
pub(crate) enum ParseError {
    OpenError(io::Error),
    ReadError(io::Error),
    EventsNotFound,
    SummaryNotFound,
    InvalidNumber(ParseIntError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenError(err) => write!(f, "Failed to open output file: {err}"),
            Self::ReadError(err) => write!(f, "Failed to read output file: {err}"),
            Self::EventsNotFound => write!(f, "Could not find the 'events' line"),
            Self::SummaryNotFound => write!(f, "Could not find the 'summary' line"),
            Self::InvalidNumber(err) => {
                write!(f, "'summary' line contained an invalid number: {err}")
            }
        }
    }
}
