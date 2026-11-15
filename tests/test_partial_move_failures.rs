mod fixtures;

use std::{fs::create_dir, path::Path};

use anyhow::Result;
use fixtures::*;
use rstest::*;

/* Trying to accept a change that depends on another move */
#[rstest]
fn test_partial_move_failures(mut layer: LayerManager) -> Result<()> {
    let dir = layer.test_filename("dir");
    let dir2 = layer.test_filename("dir2");
    let dir2_path = Path::new(&dir2);
    let file = dir2_path.join("file");

    create_dir(&dir)?;

    layer.run(&["mv", dir.as_str(), dir2.as_str()])?;
    layer.run(&["touch", file.to_str().unwrap()])?;

    layer.run(&["status", file.to_str().unwrap()])?;
    assert!(layer.xfail(&["accept", file.to_str().unwrap()]));
    //assert!(layer.last_stdout.contains("Cowardly refusing"));

    Ok(())
}
