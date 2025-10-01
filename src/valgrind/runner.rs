use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;

pub(crate) struct Cachegrind {
    out_file: Option<PathBuf>,
    allow_aslr: bool,
}

impl Cachegrind {
    pub(crate) fn new() -> Self {
        Self {
            out_file: None,
            allow_aslr: false,
        }
    }

    pub(crate) fn allow_aslr(&mut self, allow_aslr: bool) -> &mut Self {
        self.allow_aslr = allow_aslr;
        self
    }

    pub(crate) fn out_file<P: AsRef<Path>>(&mut self, out_file: P) -> &mut Self {
        self.out_file = Some(out_file.as_ref().to_owned());
        self
    }

    pub(crate) fn run<I, S>(&self, args: I) -> io::Result<ExitStatus>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut cmd = if self.allow_aslr {
            Command::new("valgrind")
        } else if cfg!(target_os = "linux") {
            let mut cmd = Command::new("setarch");
            cmd.arg("-R").arg("valgrind");
            cmd
        } else if cfg!(target_os = "freebsd") {
            let mut cmd = Command::new("proccontrol");
            cmd.arg("-m")
                .arg("aslr")
                .arg("-s")
                .arg("disable")
                .arg("valgrind");
            cmd
        } else {
            // Can't disable ASLR on this platform
            Command::new("valgrind")
        };

        cmd.arg("--tool=cachegrind")
            .arg("--cache-sim=yes")
            .arg("--instr-at-start=no");

        // Set some reasonable cache sizes. The exact sizes matter less than having fixed sizes,
        // since otherwise cachegrind would take them from the CPU and make benchmark runs
        // even more incomparable between machines.
        cmd.arg("--I1=32768,8,64")
            .arg("--D1=32768,8,64")
            .arg("--LL=8388608,16,64");

        if let Some(out_file) = &self.out_file {
            cmd.arg(format!("--cachegrind-out-file={}", out_file.display()));
        }

        for arg in args {
            cmd.arg(arg);
        }

        cmd.stdout(Stdio::null()).stderr(Stdio::null()).status()
    }

    pub(crate) fn check() -> Result<(), String> {
        let result = Command::new("valgrind")
            .arg("--tool=cachegrind")
            .arg("--version")
            .stdout(Stdio::null())
            .output();

        match result {
            Ok(out) if out.status.success() => Ok(()),
            Ok(out) => Err(format!(
                "valgrind exited with {}:\n{}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            )),
            Err(err) => Err(format!("Failed to run valgrind: {err}")),
        }
    }
}
