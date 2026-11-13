mod fixtures;

use std::process::Command;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_exercise_unmovable_files_remove(mut layer: LayerManager) -> Result<()> {
    let char_device = layer.test_filename("test-file");
    println!("setting up char_device: {}", char_device);
    let output = Command::new("sudo")
        .args(["mknod", char_device.as_str(), "c", "1", "3"])
        .output()?;
    assert!(output.status.success());

    layer.run(&["rm", char_device.as_str()])?;

    assert!(layer.xfail(&["accept", char_device.as_str()]));
    let status = layer.last_stderr.trim();
    assert!(status.contains("cowardly refusing to remove special file"));

    Ok(())
}
