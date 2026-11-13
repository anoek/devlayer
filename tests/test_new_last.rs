mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_new_last_mutual_exclusivity(mut layer: LayerManager) -> Result<()> {
    layer.no_default_options = true;

    assert!(layer.xfail(&["--new", "--last", "ls"]));
    assert!(layer.xfail(&["--join=foo", "--last", "ls"]));
    assert!(layer.xfail(&["--join=foo", "--new", "ls"]));
    assert!(layer.xfail(&["--join=foo", "--new", "--last", "ls"]));
    assert!(layer.exfail(&["--new", "ls"], "DEVLAYER_JOIN", "foo"));
    assert!(layer.exfail(&["--last", "ls"], "DEVLAYER_JOIN", "foo"));

    Ok(())
}

#[rstest]
fn test_last_is_empty(mut layer: LayerManager) -> Result<()> {
    layer.no_default_options = true;

    assert!(layer.exfail(&["--last", "-v", "true"], "TEST_LAST_IS_EMPTY", "1"));
    assert!(layer.last_stderr.contains("No layers found"));

    Ok(())
}
