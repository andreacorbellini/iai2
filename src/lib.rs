#![warn(clippy::dbg_macro)]
#![warn(missing_debug_implementations)]
#![warn(unreachable_pub)]
#![warn(unused_qualifications)]
#![doc(test(attr(deny(warnings))))]

mod macros;
mod valgrind;

use crate::valgrind::Cachegrind;
use crate::valgrind::CachegrindStats;
use crate::valgrind::parse_cachegrind_output;
use std::env::args;
use std::hint::black_box;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

fn run_bench(
    executable: &str,
    i: isize,
    name: &str,
    allow_aslr: bool,
) -> (CachegrindStats, Option<CachegrindStats>) {
    let output_file = PathBuf::from(format!("target/iai/cachegrind.out.{}", name));
    let old_file = output_file.with_file_name(format!("cachegrind.out.{}.old", name));
    std::fs::create_dir_all(output_file.parent().unwrap()).expect("Failed to create directory");

    // If this benchmark was already run once, move the last results to .old
    match std::fs::rename(&output_file, &old_file) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => eprintln!(
            "Failed to rename {} to {}: {}",
            output_file.display(),
            old_file.display(),
            err
        ),
    }

    let status = Cachegrind::new()
        .allow_aslr(allow_aslr)
        .out_file(&output_file)
        .run([executable, "--iai-run", &i.to_string(), name])
        .expect("Failed to run benchmark in cachegrind");

    if !status.success() {
        panic!(
            "Failed to run benchmark in cachegrind. Exit code: {}",
            status
        );
    }

    let new_stats =
        parse_cachegrind_output(&output_file).expect("Failed to parse cachegrind output");
    let old_stats = parse_cachegrind_output(&old_file).ok();

    (new_stats, old_stats)
}

/// Custom-test-framework runner. Should not be called directly.
#[must_use]
#[doc(hidden)]
pub fn runner(benches: &[(&'static str, fn(&'_ mut Iai))]) -> ExitCode {
    let mut args_iter = args();
    let executable = args_iter.next().unwrap();

    if let Some("--iai-run") = args_iter.next().as_deref() {
        if !valgrind::running_on_valgrind() {
            eprintln!("warning: not running under valgrind");
        }

        // In this branch, we're running under cachegrind, so execute the benchmark as quickly as
        // possible and exit
        let index: isize = args_iter.next().unwrap().parse().unwrap();

        // -1 is used as a signal to do nothing and return. By recording an empty benchmark, we can
        // subtract out the overhead from startup and dispatching to the right benchmark.
        if index == -1 {
            Iai::new().run(|| {});
            return ExitCode::SUCCESS;
        }

        let index = index as usize;
        let f = benches[index].1;
        let mut iai = Iai::new();

        f(&mut iai);

        return ExitCode::SUCCESS;
    }

    // Otherwise we're running normally, under cargo
    if let Err(err) = Cachegrind::check() {
        eprintln!("{err}");
        eprintln!("Please ensure that valgrind is installed and on $PATH");
        return ExitCode::FAILURE;
    }

    let allow_aslr = std::env::var_os("IAI_ALLOW_ASLR").is_some();

    let (calibration, old_calibration) = run_bench(&executable, -1, "iai_calibration", allow_aslr);

    for (i, (name, _func)) in benches.iter().enumerate() {
        println!("{}", name);
        let (stats, old_stats) = run_bench(&executable, i as isize, name, allow_aslr);
        let (stats, old_stats) = (
            stats.subtract(&calibration),
            match (&old_stats, &old_calibration) {
                (Some(old_stats), Some(old_calibration)) => {
                    Some(old_stats.subtract(old_calibration))
                }
                _ => None,
            },
        );

        fn signed_short(n: f64) -> String {
            let n_abs = n.abs();

            if n_abs < 10.0 {
                format!("{:+.6}", n)
            } else if n_abs < 100.0 {
                format!("{:+.5}", n)
            } else if n_abs < 1000.0 {
                format!("{:+.4}", n)
            } else if n_abs < 10000.0 {
                format!("{:+.3}", n)
            } else if n_abs < 100000.0 {
                format!("{:+.2}", n)
            } else if n_abs < 1000000.0 {
                format!("{:+.1}", n)
            } else {
                format!("{:+.0}", n)
            }
        }

        fn percentage_diff(new: u64, old: u64) -> String {
            if new == old {
                return " (No change)".to_owned();
            }

            let new: f64 = new as f64;
            let old: f64 = old as f64;

            let diff = (new - old) / old;
            let pct = diff * 100.0;

            format!(" ({:>+6}%)", signed_short(pct))
        }

        println!(
            "  Instructions:     {:>15}{}",
            stats.instruction_reads,
            match &old_stats {
                Some(old) => percentage_diff(stats.instruction_reads, old.instruction_reads),
                None => "".to_owned(),
            }
        );
        let summary = stats.summarize();
        let old_summary = old_stats.map(|stat| stat.summarize());
        println!(
            "  L1 Accesses:      {:>15}{}",
            summary.l1_hits,
            match &old_summary {
                Some(old) => percentage_diff(summary.l1_hits, old.l1_hits),
                None => "".to_owned(),
            }
        );
        println!(
            "  L2 Accesses:      {:>15}{}",
            summary.l3_hits,
            match &old_summary {
                Some(old) => percentage_diff(summary.l3_hits, old.l3_hits),
                None => "".to_owned(),
            }
        );
        println!(
            "  RAM Accesses:     {:>15}{}",
            summary.ram_hits,
            match &old_summary {
                Some(old) => percentage_diff(summary.ram_hits, old.ram_hits),
                None => "".to_owned(),
            }
        );
        println!(
            "  Estimated Cycles: {:>15}{}",
            summary.cycles(),
            match &old_summary {
                Some(old) => percentage_diff(summary.cycles(), old.cycles()),
                None => "".to_owned(),
            }
        );
        println!();
    }

    ExitCode::SUCCESS
}

#[derive(Debug)]
pub struct Iai {}

impl Iai {
    fn new() -> Self {
        Self {}
    }

    /// Runs and measures the given closure.
    ///
    /// The result of the closure is returned. This implies that, if the return type implements
    /// [`Drop`], the overhead of the `Drop` implementation is not measured.
    pub fn run<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        valgrind::start_instrumentation();
        let result = black_box(f());
        valgrind::stop_instrumentation();
        result
    }
}
