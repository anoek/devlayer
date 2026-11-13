mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

/* This test runs `clean` against the default storage directory, so it will
 * also delete any real stopped ephemeral layers older than a week. */

#[rstest]
#[ignore = "Ignoring as this will delete old non test ephemeral layers"]
fn test_clean_deletes_running_test_layers() -> Result<()> {
    let mut layer = layer();
    let name = layer.name.clone();

    layer.run(&["true"])?;
    layer.run(&["list"])?;
    assert!(layer.last_stdout.contains("Running layers:"));
    assert!(layer.last_stdout.contains(&name));

    layer.run(&["clean", "-y"])?;
    assert!(layer.last_stdout.contains(&name));

    layer.run(&["list"])?;
    assert!(!layer.last_stdout.contains(&name));

    Ok(())
}
