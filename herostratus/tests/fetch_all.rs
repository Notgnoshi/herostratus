use herostratus_tests::cmd::{CommandExt, herostratus};
use herostratus_tests::fixtures::repository::Builder;

#[test]
fn add_and_fetch() {
    // 1. Create an upstream repo
    let temp_upstream_repo = Builder::new().commit("Initial commit").build().unwrap();
    let url = format!("file://{}", temp_upstream_repo.tempdir.path().display());

    // 2. Add it to herostratus, skipping the clone
    let (mut cmd, temp_data) = herostratus(None, None);
    let data_dir = temp_data.as_ref().unwrap().path();
    cmd.arg("add").arg("--skip-clone").arg(url);
    let output = cmd.captured_output();
    assert!(output.status.success());

    // 3. Fetch the repo, which clones it under the hood, since it doesn't already exist
    let (mut cmd, _) = herostratus(Some(data_dir), None);
    cmd.arg("fetch-all");
    let output = cmd.captured_output();
    assert!(output.status.success());

    // 4. Add a commit to the upstream
    temp_upstream_repo.commit("Second commit").create().unwrap();

    // 5. Fetch the new commit
    let (mut cmd, _) = herostratus(Some(data_dir), None);
    cmd.arg("fetch-all");
    let output = cmd.captured_output();
    assert!(output.status.success());
}
