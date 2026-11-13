mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::{Path, PathBuf};

#[rstest]
fn test_delete_single_layer() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Verify layer exists
    layer.run(&["list"])?;
    assert!(layer.last_stdout.contains(&name));

    // Delete with confirmation (use -y flag)
    layer.run(&["delete", "-y", &name])?;
    assert!(layer.last_stdout.contains(&name));
    assert!(layer.last_stdout.contains("1 layers deleted"));

    // Verify layer no longer exists
    layer.run(&["list"])?;
    assert!(!layer.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_current_layer_without_y() -> Result<()> {
    let mut layer = layer();
    let _name = layer.name.clone();

    // Create and run the layer
    layer.run(&["true"])?;

    // Delete without patterns and without -y should prompt for current layer
    // TODO: Once the bug is fixed, this should show the current layer
    // For now it will show "No default layer found"
    layer.run_with_stdin(&["delete"], "n\n")?;

    assert!(
        layer
            .last_stdout
            .contains("The following layers will be deleted")
    );
    assert!(layer.last_stdout.contains(&layer.name));

    Ok(())
}

#[rstest]
fn test_delete_current_layer_no_pattern() -> Result<()> {
    let mut layer = layer();
    let _name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Delete without pattern should delete current layer
    // NOTE: There's a bug in the current implementation where patterns.is_empty()
    // is checked twice with contradictory logic, so it currently shows "No default layer"
    layer.run(&["delete", "-y"])?;

    // TODO: Once the bug is fixed, this should delete the current layer
    // For now, we test what it actually does
    assert!(layer.last_stdout.contains("1 layers deleted"));

    Ok(())
}

#[rstest]
fn test_delete_current_non_existent_layer() -> Result<()> {
    let mut layer = layer();
    let _name = layer.name.clone();

    layer.run_with_stdin(&["delete"], "y\n")?;
    assert!(layer.last_stdout.contains("No layer by the name"));

    Ok(())
}

#[rstest]
fn test_delete_shows_layer_status() -> Result<()> {
    let mut layer1 = layer();
    let mut layer2 = layer();

    // Give them unique but similar names
    let base_name = format!("devlayer-coverage-test-del-status-{}", rid());
    layer1.name = format!("{}-1", base_name);
    layer2.name = format!("{}-2", base_name);
    let name1 = layer1.name.clone();
    let name2 = layer2.name.clone();

    // Create running layer
    layer1.run(&["true"])?;

    // Create and stop another layer
    layer2.run(&["true"])?;
    layer2.run(&["stop", &name2])?;

    // Delete both with specific pattern
    layer1.run(&["delete", "-y", &format!("{}*", base_name)])?;

    // Check output shows status information
    assert!(layer1.last_stdout.contains(&name1));
    assert!(layer1.last_stdout.contains(&name2));
    // The actual output format shows "running" or "stopped" before the layer name
    assert!(layer1.last_stdout.contains("2 layers deleted"));

    Ok(())
}

#[rstest]
fn test_delete_multiple_layers_with_pattern() -> Result<()> {
    let mut layer1 = layer();
    let mut layer2 = layer();
    let mut layer3 = layer();

    // Give them related names
    layer1.name = format!("devlayer-coverage-test-delete-{}", rid());
    layer2.name = format!("devlayer-coverage-test-delete-{}", rid());
    layer3.name = format!("devlayer-coverage-test-other-{}", rid());

    // Create layers
    layer1.run(&["true"])?;
    layer2.run(&["true"])?;
    layer3.run(&["true"])?;

    // Delete layers matching pattern
    layer1.run(&["delete", "-y", "devlayer-coverage-test-delete*"])?;

    // Should show the layers being deleted
    assert!(layer1.last_stdout.contains(&layer1.name));
    assert!(layer1.last_stdout.contains(&layer2.name));
    assert!(!layer1.last_stdout.contains(&layer3.name));

    // Count how many were actually deleted (at least 2)
    let deleted_count = layer1
        .last_stdout
        .lines()
        .filter(|line| line.contains("Deleted layer:"))
        .count();
    assert!(deleted_count >= 2);

    // Verify only matching layers were deleted
    layer1.run(&["list"])?;
    assert!(!layer1.last_stdout.contains(&layer1.name));
    assert!(!layer1.last_stdout.contains(&layer2.name));
    assert!(layer1.last_stdout.contains(&layer3.name));

    Ok(())
}

#[rstest]
fn test_delete_no_matching_layers() -> Result<()> {
    let mut layer = layer();

    // Try to delete non-existent layer
    layer.run(&["delete", "-y", "non-existent-layer"])?;
    assert!(layer.last_stdout.contains("No layers found matching"));

    Ok(())
}

#[rstest]
fn test_delete_stopped_layer() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Stop the layer
    layer.run(&["stop"])?;
    layer.run(&["accept", "."])?;

    // Delete the stopped layer
    layer.run_with_stdin(&["delete"], "y\n")?;
    println!("{}", layer.last_stdout);
    assert!(layer.last_stdout.contains("0 entries"));
    assert!(layer.last_stdout.contains("stopped"));
    assert!(layer.last_stdout.contains(&name));
    assert!(layer.last_stdout.contains("1 layers deleted"));

    Ok(())
}

#[rstest]
fn test_delete_modified_layer_n() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();
    let filename = layer.test_filename("test_file");

    // Create and run a layer
    layer.run(&["touch", &filename])?;

    // Delete the stopped layer
    layer.run_with_stdin(&["delete", &name], "n\n")?;
    eprintln!("{}", layer.last_stdout);
    assert!(layer.last_stdout.contains("3 entries"));
    assert!(layer.last_stdout.contains("0 ignored"));
    assert!(layer.last_stdout.contains("running"));
    assert!(layer.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_modified_ignored_layer_n() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();
    let filename = layer.test_filename("test_file");

    // Create and run a layer
    layer.run(&["touch", &filename])?;

    // Delete the stopped layer
    layer.set_ignored(false);
    layer.run_with_stdin(&["delete", &name], "n\n")?;
    eprintln!("{}", layer.last_stdout);
    assert!(layer.last_stdout.contains("0 entries"));
    assert!(layer.last_stdout.contains("3 ignored"));
    assert!(layer.last_stdout.contains("running"));
    assert!(layer.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_stopped_layer_by_name() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Stop the layer
    layer.run(&["stop"])?;
    layer.run(&["accept", "."])?;

    // Delete the stopped layer
    layer.run_with_stdin(&["delete", &name], "y\n")?;
    assert!(layer.last_stdout.contains("0 entries"));
    assert!(layer.last_stdout.contains("stopped"));
    assert!(layer.last_stdout.contains(&name));
    assert!(layer.last_stdout.contains("1 layers deleted"));

    Ok(())
}

#[rstest]
fn test_delete_failure(mut layer: LayerManager) -> Result<()> {
    layer.run(&["true"])?;
    assert!(layer.epass(&["delete", "-y"], "TEST_DELETE_FAILURE", "1"));
    assert!(layer.last_stdout.contains("Error deleting layer"));
    Ok(())
}

#[rstest]
fn test_delete_running_layer_by_name() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Accept the layer
    layer.run(&["accept", "."])?;

    // Delete the running layer
    layer.run_with_stdin(&["delete", &name], "y\n")?;
    assert!(layer.last_stdout.contains("running"));
    assert!(layer.last_stdout.contains(&name));
    assert!(layer.last_stdout.contains("1 layers deleted"));

    Ok(())
}

#[rstest]
fn test_delete_running_layer_no_modifications_no_confirm() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer without any modifications
    layer.run(&["true"])?;

    // Delete should work even though there are no modifications
    layer.run(&["delete", "-y", &name])?;
    assert!(layer.last_stdout.contains(&name));
    assert!(layer.last_stdout.contains("1 layers deleted"));

    // Verify layer no longer exists
    layer.run(&["list"])?;
    assert!(!layer.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_multiple_stopped_layers() -> Result<()> {
    let mut layer1 = layer();
    let mut layer2 = layer();
    let mut layer3 = layer();

    // Give them related names
    let base_name = format!("devlayer-coverage-test-del-stopped-{}", rid());
    layer1.name = format!("{}-1", base_name);
    layer2.name = format!("{}-2", base_name);
    layer3.name = format!("{}-3", base_name);
    let name1 = layer1.name.clone();
    let name2 = layer2.name.clone();
    let name3 = layer3.name.clone();

    // Create and stop all layers
    layer1.run(&["true"])?;
    layer1.run(&["stop", &name1])?;

    layer2.run(&["true"])?;
    layer2.run(&["stop", &name2])?;

    layer3.run(&["true"])?;
    layer3.run(&["stop", &name3])?;

    // Delete all stopped layers with pattern
    layer1.run(&["delete", "-y", &format!("{}*", base_name)])?;

    // Should show all layers as stopped
    assert!(layer1.last_stdout.contains(&name1));
    assert!(layer1.last_stdout.contains(&name2));
    assert!(layer1.last_stdout.contains(&name3));
    assert!(layer1.last_stdout.contains("3 layers deleted"));

    Ok(())
}

#[rstest]
fn test_delete_mixed_running_and_stopped_layers() -> Result<()> {
    let mut layer1 = layer();
    let mut layer2 = layer();
    let mut layer3 = layer();

    // Give them related names
    let base_name = format!("devlayer-coverage-test-del-mixed-{}", rid());
    layer1.name = format!("{}-running", base_name);
    layer2.name = format!("{}-stopped", base_name);
    layer3.name = format!("{}-also-running", base_name);
    let name1 = layer1.name.clone();
    let name2 = layer2.name.clone();
    let name3 = layer3.name.clone();

    // Create mixed state layers
    layer1.run(&["true"])?; // Keep running

    layer2.run(&["true"])?;
    layer2.run(&["stop", &name2])?; // Stop this one

    layer3.run(&["true"])?; // Keep running

    // Delete all with pattern
    layer1.run(&["delete", "-y", &format!("{}*", base_name)])?;

    // Check that output distinguishes between running and stopped
    assert!(layer1.last_stdout.contains(&name1));
    assert!(layer1.last_stdout.contains(&name2));
    assert!(layer1.last_stdout.contains(&name3));
    assert!(layer1.last_stdout.contains("3 layers deleted"));

    // Verify all are deleted
    layer1.run(&["list"])?;
    assert!(!layer1.last_stdout.contains(&name1));
    assert!(!layer1.last_stdout.contains(&name2));
    assert!(!layer1.last_stdout.contains(&name3));

    Ok(())
}

#[rstest]
fn test_delete_json_output() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create layer
    layer.run(&["true"])?;

    // Delete with JSON output
    layer.run(&["--json", "delete", "-y", &name])?;

    // Parse JSON output
    let output: serde_json::Value = serde_json::from_str(&layer.last_stdout)?;
    assert_eq!(output["status"], "success");
    assert!(
        output["deleted"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String(name))
    );
    assert!(output["errors"].as_array().unwrap().is_empty());

    Ok(())
}

#[rstest]
fn test_delete_confirmation_yes() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Test confirming with 'y'
    layer.run_with_stdin(&["delete", &name], "y\n")?;
    assert!(layer.last_stdout.contains("1 layers deleted"));
    assert!(layer.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_confirmation_no() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Test cancelling with 'n'
    layer.run_with_stdin(&["delete", &name], "n\n")?;
    assert!(layer.last_stdout.contains("Delete operation cancelled"));

    // Verify layer still exists
    layer.run(&["list"])?;
    assert!(layer.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_confirmation_capital_n() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Test cancelling with 'N'
    layer.run_with_stdin(&["delete", &name], "N\n")?;
    assert!(layer.last_stdout.contains("Delete operation cancelled"));

    // Verify layer still exists
    layer.run(&["list"])?;
    assert!(layer.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_confirmation_default_no() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create and run a layer
    layer.run(&["true"])?;

    // Test pressing just Enter (should default to No)
    layer.run_with_stdin(&["delete", &name], "\n")?;
    assert!(layer.last_stdout.contains("Delete operation cancelled"));

    // Verify layer still exists
    layer.run(&["list"])?;
    assert!(layer.last_stdout.contains(&name));

    Ok(())
}

#[rstest]
fn test_delete_cleans_all_files() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create layer
    layer.run(&["true"])?;

    // Get storage directory to verify files
    layer.run(&["config", "storage_dir"])?;
    let storage_dir = PathBuf::from(layer.last_stdout.trim());

    let lock_file = storage_dir.join(format!("{}.lock", name));
    let pid_file = storage_dir.join(format!("{}.pid", name));
    let layer_dir = storage_dir.join(&name);

    // Verify files exist before deletion
    assert!(lock_file.exists() || pid_file.exists() || layer_dir.exists());

    // Delete layer
    layer.run(&["delete", "-y", &name])?;

    // Verify all files are removed
    assert!(!lock_file.exists());
    assert!(!pid_file.exists());
    assert!(!layer_dir.exists());

    Ok(())
}

#[rstest]
fn test_delete_with_modifications() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    // Create layer and make some modifications
    layer.run(&["true"])?;
    let test_file = layer.test_filename("test_file");
    layer.run(&["sh", "-c", &format!("echo 'test' > {}", test_file)])?;

    // Delete with -y flag just shows success messages
    layer.run(&["delete", "-y", &name])?;
    assert!(layer.last_stdout.contains(&name));
    assert!(layer.last_stdout.contains("1 layers deleted"));

    Ok(())
}

#[rstest]
fn test_delete_ignore_invalid_upper_dirs(
    mut layer: LayerManager,
) -> Result<()> {
    layer.run(&["config", "storage_dir"])?;
    let storage_dir = layer.last_stdout.trim();
    let layer_upper_dir =
        Path::new(storage_dir).join(&layer.name).join("upper");

    let invalid_base32_dir = layer_upper_dir.join("@@@INVALID@@@");
    std::fs::create_dir_all(&invalid_base32_dir)?;
    std::fs::create_dir_all(invalid_base32_dir.join("subdir"))?;
    std::fs::write(invalid_base32_dir.join("subdir/file.txt"), "test")?;

    let invalid_utf8_bytes = vec![0xFF, 0xFE];
    let base32_invalid_utf8 =
        data_encoding::BASE32_NOPAD_NOCASE.encode(&invalid_utf8_bytes);
    let invalid_utf8_dir = layer_upper_dir.join(&base32_invalid_utf8);
    std::fs::create_dir_all(&invalid_utf8_dir)?;
    std::fs::create_dir_all(invalid_utf8_dir.join("subdir"))?;
    std::fs::write(invalid_utf8_dir.join("subdir/file.txt"), "test")?;

    let filename = layer.test_filename("file");
    layer.run(&["touch", &filename])?;

    // Run it
    //layer.run(&["true"])?;
    // Now delete should process these directories and skip them
    layer.run_with_stdin(&["delete"], "y\n")?;
    assert!(layer.last_stdout.contains("1 layers deleted"));
    Ok(())
}
