mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_diff_patterns(mut layer: LayerManager) -> Result<()> {
    layer.run(&["sh", "-c", "echo 'file_one' > file_one"])?;
    layer.run(&["sh", "-c", "echo 'file_two' > file_two"])?;

    // diff for all files should show both changes
    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains("file_one"));
    assert!(layer.last_stdout.contains("file_two"));

    // root diff should show both files too
    layer.run(&["diff", "/"])?;
    assert!(layer.last_stdout.contains("file_one"));
    assert!(layer.last_stdout.contains("file_two"));

    // diff filtered to only the first file should not include the second
    layer.run(&["diff", "file_one"])?;
    assert!(layer.last_stdout.contains("file_one"));
    assert!(!layer.last_stdout.contains("file_two"));

    // diff filtered to a pattern that matches nothing should have neither
    layer.run(&["diff", "non_existent_pattern"])?;
    assert!(!layer.last_stdout.contains("file_one"));
    assert!(!layer.last_stdout.contains("file_two"));

    Ok(())
}

#[rstest]
fn test_diff_operations(mut layer: LayerManager) -> Result<()> {
    let added = layer.test_filename("added_file");
    let removed = layer.test_filename("removed_file");
    let modified = layer.test_filename("modified_file");
    let moved_src = layer.test_filename("moved_dir");
    let moved_dst = layer.test_filename("moved_dir_new");

    std::fs::write(&removed, "to be removed")?;
    std::fs::write(&modified, "old content")?;
    std::fs::create_dir(&moved_src)?;

    layer.run(&["sh", "-c", &format!("echo 'added' > {}", added)])?;
    layer.run(&["rm", &removed])?;
    layer.run(&[
        "bash",
        "-c",
        &format!("echo 'new content' > {}", modified),
    ])?;
    layer.run(&["mv", &moved_src, &moved_dst])?;

    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains(&added));
    assert!(layer.last_stdout.contains(&removed));
    assert!(layer.last_stdout.contains(&modified));
    assert!(layer.last_stdout.contains("+new content"));
    assert!(layer.last_stdout.contains("### Moved"));
    assert!(layer.last_stdout.contains(&moved_src));
    assert!(layer.last_stdout.contains(&moved_dst));

    Ok(())
}
