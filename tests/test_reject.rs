mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{fs::create_dir, fs::write, path::Path};

#[rstest]
fn test_reject_file(mut layer: LayerManager) -> Result<()> {
    let filename = layer.test_filename("file");
    let path = Path::new(&filename);
    assert!(!path.exists());
    layer.run(&["touch", &filename])?;
    assert!(!path.exists());
    layer.run(&["reject", &filename])?;
    assert!(!path.exists());
    layer.run(&["accept", &filename])?;
    assert!(!path.exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_file(mut layer: LayerManager) -> Result<()> {
    let filename = layer.test_filename("file");
    let path = Path::new(&filename);
    write(path, "test")?;
    assert!(path.exists());
    layer.run(&["rm", &filename])?;
    assert!(path.exists());
    layer.run(&["reject", &filename])?;
    assert!(path.exists());
    layer.run(&["accept", &filename])?;
    assert!(path.exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_directory(mut layer: LayerManager) -> Result<()> {
    let dirname = layer.test_filename("dir");
    let path = Path::new(&dirname);
    create_dir(path)?;
    assert!(path.exists());
    layer.run(&["rm", "-r", &dirname])?;
    assert!(path.exists());
    layer.run(&["reject", &dirname])?;
    assert!(path.exists());
    layer.run(&["accept", &dirname])?;
    assert!(path.exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_filled_directory(mut layer: LayerManager) -> Result<()> {
    let dirname = layer.test_filename("dir");
    let path = Path::new(&dirname);
    create_dir(path)?;
    write(path.join("file"), "test")?;
    assert!(path.exists());
    layer.run(&["rm", "-r", &dirname])?;
    assert!(path.exists());
    layer.run(&["reject", &dirname])?;
    assert!(path.exists());
    layer.run(&["accept", &dirname])?;
    assert!(path.exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_filled_directory_with_opaque_directory_not_empty(
    mut layer: LayerManager,
) -> Result<()> {
    let dirname = layer.test_filename("dir");
    let path = Path::new(&dirname);
    create_dir(path)?;
    write(path.join("file"), "test")?;
    assert!(path.exists());
    layer.run(&["rm", "-r", &dirname])?;
    assert!(path.exists());
    layer.run(&["mkdir", &dirname])?;
    layer.run(&["touch", path.join("file2").to_str().unwrap()])?;
    assert!(layer.pass(&["reject", &dirname]));
    assert!(path.exists());
    assert!(path.join("file").exists());
    assert!(!path.join("file2").exists());
    Ok(())
}

#[rstest]
fn test_reject_remove_filled_directory_with_opaque_directory(
    mut layer: LayerManager,
) -> Result<()> {
    let dirname = layer.test_filename("dir");
    let path = Path::new(&dirname);
    create_dir(path)?;
    write(path.join("file"), "test")?;
    assert!(path.exists());
    layer.run(&["rm", "-r", &dirname])?;
    assert!(path.exists());
    layer.run(&["mkdir", &dirname])?;
    layer.run(&["touch", path.join("file2").to_str().unwrap()])?;
    layer.run(&["reject"])?;
    assert!(path.exists());
    layer.run(&["accept"])?;
    assert!(path.exists());
    assert!(path.join("file").exists());
    assert!(!path.join("file2").exists());
    std::fs::remove_file(path.join("file"))?;
    std::fs::remove_dir(&dirname)?;
    Ok(())
}
