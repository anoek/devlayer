mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_bind_mount_under_run_media(mut layer: LayerManager) -> Result<()> {
    // Create a temporary directory to bind mount
    let test_dir = layer.test_filename("bind_test");
    std::fs::create_dir(&test_dir)?;

    // Create a test file in the temp directory
    let test_file = format!("{}/test.txt", test_dir);
    std::fs::write(&test_file, "test content")?;

    if !std::path::Path::new("/run/media/devlayer-coverage-testing").exists() {
        std::fs::create_dir_all("/run/media/devlayer-coverage-testing")?;
    }

    // Try to bind mount the temp directory to /run/media/test
    // This should work with our fix, as we now create parent directories
    let success = layer.pass(&[
        "--bind",
        &format!("{}:/run/media/devlayer-coverage-testing", test_dir),
        "ls",
        "/run/media/devlayer-coverage-testing/test.txt",
    ]);

    // The command should succeed and show the test file
    assert!(
        success,
        "Failed to bind mount to /run/media/devlayer-coverage-testing: {}",
        layer.last_stderr
    );
    assert!(
        layer.last_stdout.contains("test.txt"),
        "Test file not found in bind mount"
    );

    Ok(())
}
