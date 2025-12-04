use std::path::PathBuf;

use herostratus_tests::cmd::{CommandExt, herostratus};
use predicates::prelude::*;
use predicates::str;

#[test]
fn add_self_and_then_check_all() {
    let self_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let self_dir = format!("file://{}", self_dir.display());
    let (mut cmd, temp) = herostratus(None, None);
    cmd.arg("add").arg("--skip-clone").arg(self_dir);

    let output = cmd.captured_output();
    assert!(output.status.success());

    let (mut cmd, _) = herostratus(Some(temp.as_ref().unwrap().path()), None);
    // If 'add' skips the clone, using 'fetch-all' or 'check-all' without '--no-fetch' will clone
    cmd.arg("check-all");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // who knows how many achievements 'HEAD' will have?
    let assertion =
        str::contains("Finalizing rules ...").and(str::contains("achievements after processing"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(assertion.eval(&stderr));
}
