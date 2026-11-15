mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_no_network(mut layer: LayerManager) -> Result<()> {
    assert!(layer.xfail(&[
        "--net=none",
        "ping",
        "-c",
        "1",
        "-W",
        "1",
        "127.0.0.1"
    ]));
    assert!(layer.last_stderr.contains("Creating new network namespace"));
    Ok(())
}

#[rstest]
fn test_network(mut layer: LayerManager) -> Result<()> {
    let status = layer.run(&[
        "--net=host",
        "ping",
        "-c",
        "1",
        "-W",
        "1",
        "127.0.0.1",
    ])?;
    assert!(layer.last_stderr.contains("Using host network"));
    assert!(status.status.success());
    Ok(())
}
