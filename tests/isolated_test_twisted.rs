mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{
    fs::{create_dir_all, write},
    path::{Path, PathBuf},
};

#[rstest]
fn test_twisted(mut layer: LayerManager) -> Result<()> {
    let path = PathBuf::from(&layer.test_filename_no_rid("source"));

    create_dir_all(format!("{}/a/b/c", path.display()))?;
    write(format!("{}/a/b/c/file_c", path.display()), "file_c")?;
    write(format!("{}/a/b/file_b", path.display()), "file_b")?;

    layer.run(&[
        "mkdir",
        "-p",
        format!("{}/a/b/c/d", path.display()).as_str(),
    ])?;
    layer.run(&[
        "mv",
        format!("{}/a/b/c/d", path.display()).as_str(),
        format!("{}/d", path.display()).as_str(),
    ])?;
    layer.run(&[
        "mv",
        format!("{}/a", path.display()).as_str(),
        format!("{}/d/a", path.display()).as_str(),
    ])?;
    layer.run(&[
        "cp",
        format!("{}/d/a/b/c/file_c", path.display()).as_str(),
        format!("{}/d/file_c_v2", path.display()).as_str(),
    ])?;
    layer.run(&[
        "cp",
        format!("{}/d/a/b/file_b", path.display()).as_str(),
        format!("{}/d/a/b/c/file_c", path.display()).as_str(),
    ])?;
    layer.run(&["rm", format!("{}/d/a/b/file_b", path.display()).as_str()])?;

    layer.run(&["accept"])?;

    assert!(
        std::fs::read_to_string(format!("{}/d/file_c_v2", path.display()))?
            == "file_c"
    );
    assert!(
        !Path::new(format!("{}/d/a/b/file_b", path.display()).as_str())
            .exists()
    );
    assert!(
        std::fs::read_to_string(format!("{}/d/a/b/c/file_c", path.display()))?
            == "file_b"
    );

    Ok(())
}

#[rstest]
fn test_twisted_fail_rmdir_on_separate_device(
    mut layer: LayerManager,
) -> Result<()> {
    let path = PathBuf::from(&layer.test_filename_no_rid("source"));

    create_dir_all(format!("{}/a/b/c", path.display()))?;
    write(format!("{}/a/b/c/file_c", path.display()), "file_c")?;
    write(format!("{}/a/b/file_b", path.display()), "file_b")?;

    layer.run(&[
        "mkdir",
        "-p",
        format!("{}/a/b/c/d", path.display()).as_str(),
    ])?;
    layer.run(&[
        "mv",
        format!("{}/a/b/c/d", path.display()).as_str(),
        format!("{}/d", path.display()).as_str(),
    ])?;
    layer.run(&[
        "mv",
        format!("{}/a", path.display()).as_str(),
        format!("{}/d/a", path.display()).as_str(),
    ])?;
    layer.run(&[
        "cp",
        format!("{}/d/a/b/c/file_c", path.display()).as_str(),
        format!("{}/d/file_c_v2", path.display()).as_str(),
    ])?;
    layer.run(&[
        "cp",
        format!("{}/d/a/b/file_b", path.display()).as_str(),
        format!("{}/d/a/b/c/file_c", path.display()).as_str(),
    ])?;
    layer.run(&["rm", format!("{}/d/a/b/file_b", path.display()).as_str()])?;

    assert!(layer.exfail(
        &["accept"],
        "TEST_ACCEPT_FAIL_RMDIR_ON_DIFFERENT_DEVICE",
        "1"
    ));

    Ok(())
}
