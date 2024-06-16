mod common;

use common::CommandExt;

#[test]
#[ignore = "Slow; performs git clone"]
fn clone_herostratus() {
    let (mut cmd, temp) = common::herostratus(None);
    let data_dir = temp.as_ref().unwrap().path();

    let expected_bare_repo = data_dir
        .join("git")
        .join("Notgnoshi")
        .join("herostratus.git");

    let url = "https://github.com/Notgnoshi/herostratus.git";
    cmd.arg("add").arg(url);

    assert!(!data_dir.join("git").exists());

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());
    assert!(expected_bare_repo.exists());

    // Adding the same URL again in the same data_dir fails ...
    let (mut cmd, _temp) = common::herostratus(Some(data_dir));
    cmd.arg("add").arg(url);

    let output = cmd.captured_output().unwrap();
    assert!(!output.status.success());

    // ... unless the --force flag is given
    let (mut cmd, _temp) = common::herostratus(Some(data_dir));
    cmd.arg("add").arg("--force").arg(url);

    let output = cmd.captured_output().unwrap();
    assert!(output.status.success());
}
