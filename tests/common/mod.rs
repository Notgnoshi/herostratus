use std::path::{Path, PathBuf};
use std::process::Output;

use assert_cmd::Command;
use tempfile::{tempdir, TempDir};

// Cache the path to the binary as suggested by https://github.com/assert-rs/assert_cmd/issues/6 to
// avoid expensive repeated lookups.
lazy_static::lazy_static! {
    static ref HEROSTRATUS: PathBuf = assert_cmd::cargo::cargo_bin("herostratus");
}

/// Get a [`Command`] for the herostratus binary and the [`TempDir`] data dir used in the test
pub fn herostratus(data_dir: Option<&Path>) -> (Command, Option<TempDir>) {
    let (tempdir, path) = if let Some(data_dir) = data_dir {
        (None, data_dir.to_path_buf())
    } else {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().to_path_buf();
        (Some(temp), data_dir)
    };

    let mut cmd = Command::new(&*HEROSTRATUS);
    cmd.arg("--log-level=DEBUG").arg("--data-dir").arg(path);

    (cmd, tempdir)
}

fn capture_output(output: &Output) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Test output capture relies on magic in the print! and println! macros
    print!("{stdout}");
    print!("{stderr}");
}

pub trait CommandExt {
    /// Same as [Command::output], except with hooks to print stdout and stderr for failed tests
    fn captured_output(&mut self) -> std::io::Result<Output>;
}

impl CommandExt for Command {
    fn captured_output(&mut self) -> std::io::Result<Output> {
        let output = self.output()?;
        capture_output(&output);
        Ok(output)
    }
}
