use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::LazyLock;

use tempfile::{TempDir, tempdir};

// I'm using cargo_bin to discovery a binary provided by another crate in the workspace. That's not
// something the suggested cargo_bin! replacement supports.
#[expect(deprecated)]
static HEROSTRATUS: LazyLock<PathBuf> =
    LazyLock::new(|| assert_cmd::cargo::cargo_bin("herostratus"));

/// Get a [`Command`] for the herostratus binary and the [`TempDir`] data dir used in the test
pub fn herostratus(data_dir: Option<&Path>) -> (assert_cmd::Command, Option<TempDir>) {
    let (tempdir, path) = if let Some(data_dir) = data_dir {
        (None, data_dir.to_path_buf())
    } else {
        let temp = tempdir().unwrap();
        let data_dir = temp.path().to_path_buf();
        (Some(temp), data_dir)
    };

    let mut cmd = assert_cmd::Command::new(&*HEROSTRATUS);
    cmd.arg("--color")
        .arg("--log-level=DEBUG")
        .arg("--data-dir")
        .arg(path);

    (cmd, tempdir)
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
