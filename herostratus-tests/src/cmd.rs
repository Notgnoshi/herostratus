use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::LazyLock;

use herostratus::config::{Config, read_config, write_config};
use tempfile::TempDir;

// I'm using cargo_bin to discovery a binary provided by another crate in the workspace. That's not
// something the suggested cargo_bin! replacement supports.
static HEROSTRATUS: LazyLock<PathBuf> =
    LazyLock::new(|| assert_cmd::cargo::cargo_bin("herostratus"));

/// Test harness that manages a temporary data directory and creates pre-configured commands
///
/// The harness owns a [TempDir] that serves as the herostratus data directory. It provides methods
/// to read/write config files and to create [assert_cmd::Command]s with the standard test arguments
/// (--color, --log-level=DEBUG, --data-dir).
pub struct TestHarness {
    tempdir: TempDir,
}

impl Default for TestHarness {
    fn default() -> Self {
        Self {
            tempdir: TempDir::new().unwrap(),
        }
    }
}

impl TestHarness {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn path(&self) -> &Path {
        self.tempdir.path()
    }

    /// Write a config to the data directory
    pub fn write_config(&self, config: &Config) {
        write_config(self.path(), config).unwrap();
    }

    /// Read the config, apply a transform, and write it back
    pub fn update_config(&self, f: impl FnOnce(Config) -> Config) {
        let config = read_config(self.path()).unwrap();
        let config = f(config);
        write_config(self.path(), &config).unwrap();
    }

    /// Create a [assert_cmd::Command] for the herostratus binary with standard test arguments
    pub fn command(&self) -> assert_cmd::Command {
        let mut cmd = assert_cmd::Command::new(&*HEROSTRATUS);
        cmd.arg("--color")
            .arg("--log-level=DEBUG")
            .arg("--data-dir")
            .arg(self.path());
        cmd
    }
}

pub trait CommandExt {
    /// Same as `Command::output`, except with hooks to print stdout and stderr for failed tests
    fn captured_output(&mut self) -> Output;
}

impl CommandExt for assert_cmd::Command {
    #[track_caller]
    fn captured_output(&mut self) -> Output {
        let output = self.output().unwrap();
        // libtest has hooks in the print! and eprint! macros to do output capturing in tests.
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        output
    }
}

impl CommandExt for std::process::Command {
    #[track_caller]
    fn captured_output(&mut self) -> Output {
        let output = self.output().unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        output
    }
}
