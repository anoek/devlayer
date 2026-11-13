mod fixtures;

use anyhow::{Context, Result};
use fixtures::*;
use rstest::*;
use std::path::PathBuf;

#[rstest]
fn test_sync(mut layer: LayerManager) -> Result<()> {
    layer.run(&["true"])?;
    layer.run(&["config", "upper_cwd"])?;
    let base = layer.last_stdout.trim();
    let upper_dir = PathBuf::from(base);

    let t = layer.test_filename("test-file");
    std::fs::write(&t, "hello")?;

    layer.run(&[
        "bash",
        "-c",
        format!("echo -n 'from layer echo' > {}", t.as_str()).as_str(),
    ])?;

    std::fs::write(&t, "goodbye")?;

    layer.run(&["cat", t.clone().as_str()])?;
    let stdout = layer.last_stdout.trim();
    assert_eq!(stdout, "from layer echo");

    // remove the file from the upper directory
    std::fs::remove_file(upper_dir.join(&t)).context(format!(
        "failed to remove file: {} [upper={}]",
        t,
        upper_dir.display()
    ))?;
    assert!(PathBuf::from(&t).exists());

    layer.run(&["sync"])?;

    // after a sync, programs run in the layer should see the changes. Before, the results are
    // undefined.
    layer.run(&["cat", t.clone().as_str()])?;
    let stdout = layer.last_stdout.trim();
    assert_eq!(stdout, "goodbye");

    Ok(())
}
