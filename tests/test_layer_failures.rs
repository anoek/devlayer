mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
#[rstest]
fn test_fail_to_start_layer(mut layer: LayerManager) -> Result<()> {
    assert!(layer.exfail(&["env"], "TEST_START_LAYER_FAILURE", "1"));
    assert!(layer.all_stderr.contains("Failed to start layer"));
    Ok(())
}

#[rstest]
fn test_running_as_user(mut layer: LayerManager) -> Result<()> {
    layer.no_sudo = true;
    assert!(layer.xfail(&["env"]));
    assert!(layer.all_stderr.contains("Insufficient permissions"));
    layer.no_sudo = false; // so teardown can run without errors
    Ok(())
}

#[rstest]
fn test_recover_from_bad_pid_file() -> Result<()> {
    let mut s = layer();
    s.run(&["config", "layer_dir"])?;
    let storage_dir = String::from(s.last_stdout.trim());
    let mut storage_dir = PathBuf::from(&storage_dir);
    storage_dir.pop();

    let pid_file_name = format!("{}/{}.pid", storage_dir.display(), s.name);
    std::fs::write(&pid_file_name, "not a number")?;
    println!("pid_file_name: {}", &pid_file_name);
    let pid_file = std::fs::File::open(&pid_file_name)?;
    let pid_file_content = std::io::read_to_string(pid_file)?;
    println!("pid_file_content: {}", pid_file_content);

    assert!(s.run(&["true"]).is_ok());

    let pid_file = std::fs::File::open(&pid_file_name)?;
    let pid_file_content = std::io::read_to_string(pid_file)?;
    println!("pid_file: {} contains {}", pid_file_name, pid_file_content);
    assert!(pid_file_content.parse::<i32>().is_ok());
    Ok(())
}

#[rstest]
fn test_lock(mut layer: LayerManager) -> Result<()> {
    layer.run(&["config", "layer_dir"])?;
    let layer_dir = String::from(layer.last_stdout.trim());
    let lock_file = format!("{}.lock", layer_dir);
    println!("lock_file: {}", lock_file);
    Command::new("sudo")
        .args(["rm", "-f", lock_file.as_str()])
        .output()?;
    std::fs::create_dir_all(&lock_file)?;
    // lock file is now a directory, should fail
    assert!(layer.xfail(&["true"]));
    assert!(layer.all_stderr.contains("Failed to open lock"));

    // remove directory
    std::fs::remove_dir(&lock_file)?;
    // lock file is now a file, should succeed
    assert!(layer.pass(&["true"]));

    Ok(())
}

#[rstest]
fn test_unable_to_join_layer(mut layer: LayerManager) -> Result<()> {
    assert!(layer.exfail(&["true"], "TEST_UNABLE_TO_JOIN_LAYER", "1"));
    assert!(
        layer
            .last_stderr
            .contains("Failed to join layer namespaces")
    );

    Ok(())
}

#[rstest]
fn test_rename_across_mount_points_failure(
    mut layer: LayerManager,
) -> Result<()> {
    let test_file = layer.test_filename("test-file");
    let test_file2 = layer.test_filename("test-file2");
    std::fs::create_dir_all(&test_file)?;
    layer.run(&["mv", test_file.as_str(), test_file2.as_str()])?;
    assert!(layer.exfail(
        &["accept", "."],
        "TEST_RENAME_ACROSS_MOUNT_POINTS_FAILURE",
        "1"
    ));
    assert!(layer.last_stderr.contains("crosses a mount point"));
    Ok(())
}

#[rstest]
fn test_bad_staged_file(mut layer: LayerManager) -> Result<()> {
    let test_file = layer.test_filename("test-file");
    layer.run(&["touch", test_file.as_str()])?;
    assert!(layer.exfail(&["accept", "."], "TEST_NO_STAGED_FILE", "1"));

    let test_file = layer.test_filename("test-file");
    layer.run(&["touch", test_file.as_str()])?;
    assert!(layer.exfail(&["accept", "."], "TEST_BAD_STAGED_FILE", "1"));
    assert!(!Path::new(test_file.as_str()).exists());
    Ok(())
}

#[rstest]
fn test_bad_staged_file_remove(mut layer: LayerManager) -> Result<()> {
    let test_file = layer.test_filename("test-file");
    std::fs::write(&test_file, "test")?;
    layer.run(&["rm", test_file.as_str()])?;
    assert!(layer.exfail(&["accept", "."], "TEST_NO_STAGED_FILE", "1"));

    let test_file = layer.test_filename("test-file");
    layer.run(&["touch", test_file.as_str()])?;
    assert!(layer.exfail(&["accept", "."], "TEST_BAD_STAGED_FILE", "1"));
    assert!(!Path::new(test_file.as_str()).exists());
    Ok(())
}

#[rstest]
fn test_remove_underlying_dir_failure(mut layer: LayerManager) -> Result<()> {
    let test_dir = layer.test_filename("test-dir");
    let test_dir2 = layer.test_filename("test-dir2");
    std::fs::create_dir_all(&test_dir)?;
    layer.run(&["mv", test_dir.as_str(), test_dir2.as_str()])?;
    std::fs::remove_dir(&test_dir)?;
    assert!(layer.pass(&["status", "."]));
    let last_stdout = layer.last_stdout.clone();
    assert!(last_stdout.contains("Redirect path not found"));

    assert!(layer.xfail(&["accept", "."]));
    assert!(last_stdout.contains("Redirect path not found"));

    Ok(())
}
