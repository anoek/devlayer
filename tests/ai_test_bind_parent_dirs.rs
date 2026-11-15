mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn ai_test_bind_parent_dir_missing_on_host(
    mut layer: LayerManager,
) -> Result<()> {
    let test_dir = layer.test_filename("bind_parent");
    std::fs::create_dir_all(&test_dir)?;
    std::fs::write(format!("{}/test.txt", test_dir), "test content")?;

    // Neither the target nor its parent exist on the host, so the parent is
    // created inside the layer without any host ownership to mirror
    let parent = format!("/run/{}", layer.name);
    assert!(!std::path::Path::new(&parent).exists());
    let target = format!("{}/mnt", parent);

    let bind = format!("{}:{}", test_dir, target);

    layer.run(&[
        "--no-config",
        "--bind",
        &bind,
        "stat",
        "-c",
        "%u",
        &parent,
    ])?;
    assert_eq!(layer.last_stdout.trim(), "0");

    layer.run(&[
        "--no-config",
        "--bind",
        &bind,
        "cat",
        &format!("{}/test.txt", target),
    ])?;
    assert_eq!(layer.last_stdout.trim(), "test content");

    Ok(())
}
