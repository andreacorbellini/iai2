#![warn(clippy::dbg_macro)]
#![warn(missing_debug_implementations)]
#![warn(unreachable_pub)]
#![warn(unused_qualifications)]
#![warn(unused_crate_dependencies)]
#![doc(test(attr(deny(warnings))))]

mod cachegrind;
mod macros;

use crate::cachegrind::Cachegrind;
use crate::cachegrind::CachegrindStats;
use crate::cachegrind::parse_cachegrind_output;
use clap::Parser;
use std::convert::Infallible;
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs;
use std::hint::black_box;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;

macro_rules! warn {
     ( $( $tt:tt )* ) => {{
         ::std::eprint!("warning: ");
         ::std::eprintln!($( $tt )*)
     }}
}

macro_rules! error {
     ( $( $tt:tt )* ) => {{
         ::std::eprint!("error: ");
         ::std::eprintln!($( $tt )*)
     }}
}

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[arg(long)]
    bench: bool,

    #[arg(long)]
    #[doc(hidden)]
    iai_run: Option<Benchmark>,
}

#[derive(Clone, Debug)]
enum Benchmark {
    User(String),
    Calibration,
}

impl Benchmark {
    fn name(&self) -> &str {
        match self {
            Self::User(name) => name,
            Self::Calibration => "::iai::calibration",
        }
    }
}

impl FromStr for Benchmark {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == Self::Calibration.name() {
            Ok(Self::Calibration)
        } else {
            Ok(Self::User(s.to_string()))
        }
    }
}

#[derive(Clone, Debug)]
struct Stats {
    new: CachegrindStats,
    old: Option<CachegrindStats>,
}

impl Stats {
    fn subtract(&self, other: &Self) -> Self {
        let new = self.new.subtract(&other.new);
        let old = match (&self.old, &other.old) {
            (Some(a), Some(b)) => Some(a.subtract(b)),
            _ => None,
        };
        Self { new, old }
    }
}

#[derive(Clone, Debug)]
struct BenchRunner {
    executable: OsString,
    allow_aslr: bool,
}

impl BenchRunner {
    fn new<S: AsRef<OsStr>>(executable: S) -> Self {
        Self {
            executable: executable.as_ref().to_owned(),
            allow_aslr: false,
        }
    }

    fn allow_aslr(&mut self, allow_aslr: bool) -> &mut Self {
        self.allow_aslr = allow_aslr;
        self
    }

    fn run(&mut self, benchmark: &Benchmark) -> Result<Stats, Box<dyn Error>> {
        let name = benchmark.name();

        let target_dir =
            PathBuf::from(env::var_os("CARGO_TARGET_DIR").unwrap_or_else(|| "target".into()));
        let iai_dir = target_dir.join("iai");
        let output_file = iai_dir.join(format!("cachegrind.out.{}", name));
        let old_file = iai_dir.join(format!("cachegrind.out.{}.old", name));

        fs::create_dir_all(&iai_dir)
            .map_err(|err| format!("Failed to create directory {}: {}", iai_dir.display(), err))?;

        // If this benchmark was already run once, move the last results to .old
        match fs::rename(&output_file, &old_file) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => warn!(
                "Failed to rename {} to {}: {}",
                output_file.display(),
                old_file.display(),
                err
            ),
        }

        Cachegrind::new()
            .allow_aslr(self.allow_aslr)
            .out_file(&output_file)
            .run([&self.executable, &format!("--iai-run={name}").into()])
            .map_err(|err| format!("Failed to run benchmark {name} in cachegrind: {err}"))?;

        let new = parse_cachegrind_output(&output_file).map_err(|err| {
            format!("Failed to parse cachegrind output for benchmark {name}: {err}")
        })?;
        let old = parse_cachegrind_output(&old_file).ok();

        Ok(Stats { new, old })
    }
}

type UserBenchmarks = [(&'static str, fn(&'_ mut Iai))];

/// Custom-test-framework runner. Should not be called directly.
#[must_use]
#[doc(hidden)]
pub fn runner(benches: &UserBenchmarks) -> ExitCode {
    let args = Args::parse();

    let result = if let Some(ref bench) = args.iai_run {
        // We've been asked to run a single benchmark under valgrind
        run_benchmark(benches, bench)
    } else {
        // Otherwise we're running normally under cargo
        run_all_benchmarks(benches)
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run_benchmark(benches: &UserBenchmarks, bench: &Benchmark) -> Result<(), Box<dyn Error>> {
    if !cachegrind::running_on_valgrind() {
        warn!("Not running under valgrind");
    }

    match bench {
        Benchmark::User(name) => {
            let f = benches
                .iter()
                .find_map(|(name, f)| if *name == bench.name() { Some(f) } else { None })
                .ok_or_else(|| format!("no benchmark function with name: {name}"))?;
            let mut iai = Iai::new();
            f(&mut iai);
            Ok(())
        }
        Benchmark::Calibration => {
            Iai::new().run(|| {});
            Ok(())
        }
    }
}

fn run_all_benchmarks(benches: &UserBenchmarks) -> Result<(), Box<dyn Error>> {
    let executable = env::args_os().next().expect("first argument is missing");

    Cachegrind::check()
        .map_err(|err| format!("{err}\nPlease ensure that valgrind is installed and on $PATH"))?;

    let mut runner = BenchRunner::new(executable);
    runner.allow_aslr(env::var_os("IAI_ALLOW_ASLR").is_some());

    let calibration_stats = runner.run(&Benchmark::Calibration)?;

    for (name, _func) in benches.iter() {
        println!("{}", name);
        let stats = runner.run(&Benchmark::User(name.to_string()))?;

        let stats = stats.subtract(&calibration_stats);

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
            stats.new.instruction_reads,
            match &stats.old {
                Some(old) => percentage_diff(stats.new.instruction_reads, old.instruction_reads),
                None => "".to_owned(),
            }
        );

        let summary = stats.new.summarize();
        let old_summary = stats.old.map(|stat| stat.summarize());
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

    Ok(())
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
        cachegrind::start_instrumentation();
        let result = black_box(f());
        cachegrind::stop_instrumentation();
        result
    }
}
