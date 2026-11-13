mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_disable_fuse(mut layer: LayerManager) -> Result<()> {
    assert!(layer.xfail(&["--bind-fuse=false", "ls", "-l", "/dev/fuse"]));
    Ok(())
}
