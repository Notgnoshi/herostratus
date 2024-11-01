mod common;

use common::CommandExt;
use predicates::prelude::*;
use predicates::str;

#[test]
fn add_self_and_then_check_all() {
    let self_dir = format!("file://{}/..", env!("CARGO_MANIFEST_DIR"));
    let (mut cmd, temp) = common::herostratus(None);
    cmd.arg("add").arg("--skip-clone").arg(self_dir);

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());

    let (mut cmd, _) = common::herostratus(Some(temp.as_ref().unwrap().path()));
    // If 'add' skips the clone, using 'fetch-all' or 'check-all' without '--no-fetch' will clone
    cmd.arg("check-all");
    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());

    // who knows how many achievements 'HEAD' will have?
    let assertion =
        str::contains("Finalizing rules ...").and(str::contains("achievements after processing"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(assertion.eval(&stderr));
}
