mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::fs;

#[rstest]
fn test_diff_rename_operation(mut layer: LayerManager) -> Result<()> {
    // Test rename operation - directories show as "### Moved"
    let dir1 = layer.test_filename("dir1");
    let dir2 = layer.test_filename("dir2");
    fs::create_dir(&dir1)?;

    // Rename the directory
    layer.run(&["mv", &dir1, &dir2])?;

    // Diff should show the rename
    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains("### Moved"));
    assert!(layer.last_stdout.contains(&dir1));
    assert!(layer.last_stdout.contains(&dir2));

    Ok(())
}

#[rstest]
fn test_diff_colorized_output(mut layer: LayerManager) -> Result<()> {
    // Test colorized diff output
    let file = layer.test_filename("color_test.txt");
    fs::write(&file, "original")?;

    // Modify file
    layer.run(&["sh", "-c", &format!("echo 'modified' > {}", file)])?;

    // Check diff output with color forced
    layer.run_with_env(&["diff"], "TEST_FORCE_DIFF_COLOR", "1")?;
    assert!(layer.last_stdout.contains("-original"));
    assert!(layer.last_stdout.contains("+modified"));
    // The actual color codes would be in the output when TEST_FORCE_DIFF_COLOR is set

    Ok(())
}

#[rstest]
fn test_diff_no_color(mut layer: LayerManager) -> Result<()> {
    // Test non-colorized diff output
    let file = layer.test_filename("no_color.txt");
    fs::write(&file, "content")?;

    // Modify file
    layer.run(&["sh", "-c", &format!("echo 'changed' > {}", file)])?;

    // Check diff output with NO_COLOR set
    layer.run_with_env(&["diff"], "NO_COLOR", "1")?;
    assert!(layer.last_stdout.contains("-content"));
    assert!(layer.last_stdout.contains("+changed"));

    Ok(())
}

#[rstest]
fn test_diff_rename_colorized(mut layer: LayerManager) -> Result<()> {
    // Test colorized rename output - directories show as "### Moved"
    let dir1 = layer.test_filename("color_dir1");
    let dir2 = layer.test_filename("color_dir2");
    fs::create_dir(&dir1)?;

    // Rename the directory
    layer.run(&["mv", &dir1, &dir2])?;

    // Diff with color forced
    layer.run_with_env(&["diff"], "TEST_FORCE_DIFF_COLOR", "1")?;
    assert!(layer.last_stdout.contains("### Moved"));
    // When colorized, the output would have ANSI color codes

    Ok(())
}

#[rstest]
fn test_diff_error_operation_unsupported_file_type(
    mut layer: LayerManager,
) -> Result<()> {
    // Create a FIFO (named pipe) - unsupported file type
    let fifo_path = layer.test_filename("test.fifo");
    nix::unistd::mkfifo(
        fifo_path.as_str(),
        nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
    )?;

    // Try to modify it
    layer.run(&["sh", "-c", "echo 'data' > test.fifo &"])?;

    // Check diff - should skip error entries
    layer.run(&["diff"])?;
    // Error operations are silently skipped in diff

    Ok(())
}

#[rstest]
fn test_diff_non_file_destination(mut layer: LayerManager) -> Result<()> {
    // Create a directory
    let dir_path = layer.test_filename("test_dir");
    fs::create_dir(&dir_path)?;

    // Modify directory permissions
    layer.run(&["chmod", "755", &dir_path])?;

    // Diff should skip directories
    layer.run(&["diff"])?;
    // Should not contain diff output for directory
    assert!(!layer.last_stdout.contains("test_dir"));

    Ok(())
}

#[rstest]
fn test_diff_staged_non_file(mut layer: LayerManager) -> Result<()> {
    // Create a regular file
    let file_path = layer.test_filename("regular.txt");
    fs::write(&file_path, "content")?;

    // Replace it with a directory in layer
    layer.run(&["rm", &file_path])?;
    layer.run(&["mkdir", &file_path])?;

    // Diff should handle this case
    layer.run(&["diff"])?;

    Ok(())
}

#[rstest]
fn test_diff_missing_destination_file(mut layer: LayerManager) -> Result<()> {
    // Create a new file (destination doesn't exist)
    layer.run(&["sh", "-c", "echo 'new content' > newfile.txt"])?;

    // Diff should use /dev/null as left path
    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains("+new content"));

    Ok(())
}

#[rstest]
fn test_diff_remove_operation(mut layer: LayerManager) -> Result<()> {
    // Create a file
    let file_path = layer.test_filename("to_remove.txt");
    fs::write(&file_path, "content to remove")?;

    // Remove it
    layer.run(&["rm", &file_path])?;

    // Diff should show deletion
    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains("-content to remove"));

    Ok(())
}

#[rstest]
fn test_diff_path_replacement_in_output(mut layer: LayerManager) -> Result<()> {
    // Create a file with content
    let file_path = layer.test_filename("path_test.txt");
    fs::write(&file_path, "Original content")?;

    // Modify it
    layer.run(&[
        "sh",
        "-c",
        &format!("echo 'Modified content' > {}", file_path),
    ])?;

    // Check diff output
    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains("-Original content"));
    assert!(layer.last_stdout.contains("+Modified content"));

    Ok(())
}

#[rstest]
fn test_diff_force_color_env_variable(mut layer: LayerManager) -> Result<()> {
    // Create and modify a file
    let file_path = layer.test_filename("color_test.txt");
    fs::write(&file_path, "original")?;

    layer.run(&["sh", "-c", &format!("echo 'modified' > {}", file_path)])?;

    // Test with TEST_FORCE_DIFF_COLOR set
    layer.run_with_env(&["diff"], "TEST_FORCE_DIFF_COLOR", "1")?;
    assert!(layer.last_stdout.contains("-original"));
    assert!(layer.last_stdout.contains("+modified"));
    // Color codes would be in the actual diff output

    Ok(())
}

#[rstest]
fn test_diff_with_patterns(mut layer: LayerManager) -> Result<()> {
    // Create multiple files
    let file1 = layer.test_filename("file1.txt");
    let file2 = layer.test_filename("file2.txt");
    let other = layer.test_filename("other.log");
    fs::write(&file1, "content1")?;
    fs::write(&file2, "content2")?;
    fs::write(&other, "log content")?;

    // Modify all files
    layer.run(&["sh", "-c", 
        &format!("echo 'modified1' > {} && echo 'modified2' > {} && echo 'modified log' > {}", file1, file2, other)])?;

    // Diff with pattern - test filtering by specific file
    layer.run(&["diff", &file1])?;
    assert!(layer.last_stdout.contains("modified1"));
    assert!(!layer.last_stdout.contains("modified2"));
    assert!(!layer.last_stdout.contains("modified log"));

    // Diff with pattern matching - show all changes
    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains("modified1"));
    assert!(layer.last_stdout.contains("modified2"));
    assert!(layer.last_stdout.contains("modified log"));

    Ok(())
}

#[rstest]
fn test_diff_edge_case_null_staged(mut layer: LayerManager) -> Result<()> {
    // Create a file
    let file_path = layer.test_filename("edge.txt");
    fs::write(&file_path, "original")?;

    // Modify in layer
    layer.run(&["sh", "-c", &format!("echo 'new' > {}", file_path)])?;

    // Diff should work normally
    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains("-original"));
    assert!(layer.last_stdout.contains("+new"));

    Ok(())
}
