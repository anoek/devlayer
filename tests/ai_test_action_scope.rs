mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::path::PathBuf;

/// Creates `<test dir>/a` and `<test dir>/b`, runs the layer from `a`, and
/// writes `b/<file>` inside the layer. Returns the host path of that file.
fn write_outside_cwd(
    layer: &mut LayerManager,
    file: &str,
    contents: &str,
) -> Result<PathBuf> {
    let root = std::env::current_dir()?.join(layer.test_filename("scope"));
    std::fs::create_dir_all(root.join("a"))?;
    std::fs::create_dir_all(root.join("b"))?;
    layer.cwd = Some(root.join("a"));
    let path = root.join("b").join(file);
    let script = format!("echo {} > {}", contents, path.display());
    layer.run(&["sh", "-c", &script])?;
    Ok(path)
}

#[rstest]
fn ai_test_status_default_whole_layer(mut layer: LayerManager) -> Result<()> {
    write_outside_cwd(&mut layer, "scope-status-file", "x")?;
    layer.run(&["status"])?;
    assert!(layer.last_stdout.contains("scope-status-file"));
    Ok(())
}

#[rstest]
fn ai_test_diff_default_whole_layer(mut layer: LayerManager) -> Result<()> {
    write_outside_cwd(&mut layer, "scope-diff-file", "scope-diff-contents")?;
    layer.run(&["diff"])?;
    assert!(layer.last_stdout.contains("scope-diff-contents"));
    Ok(())
}

#[rstest]
fn ai_test_status_diff_patterns_still_filter(
    mut layer: LayerManager,
) -> Result<()> {
    write_outside_cwd(
        &mut layer,
        "scope-filter-file",
        "scope-filter-contents",
    )?;

    layer.run(&["status", "./nothing-here"])?;
    assert!(!layer.last_stdout.contains("scope-filter-file"));
    layer.run(&["status", "../b"])?;
    assert!(layer.last_stdout.contains("scope-filter-file"));

    layer.run(&["diff", "./nothing-here"])?;
    assert!(!layer.last_stdout.contains("scope-filter-contents"));
    layer.run(&["diff", "../b"])?;
    assert!(layer.last_stdout.contains("scope-filter-contents"));
    Ok(())
}

#[rstest]
fn ai_test_accept_requires_pattern(mut layer: LayerManager) -> Result<()> {
    let path = write_outside_cwd(&mut layer, "scope-accept-file", "x")?;

    assert!(layer.xfail(&["accept"]));
    assert!(layer.last_stderr.contains(
        "accept requires at least one pattern; use `devlayer accept /` to accept every change in the layer"
    ));

    // Nothing was accepted
    assert!(!path.exists());
    layer.run(&["status", "/"])?;
    assert!(layer.last_stdout.contains("scope-accept-file"));
    Ok(())
}

#[rstest]
fn ai_test_reject_requires_pattern(mut layer: LayerManager) -> Result<()> {
    write_outside_cwd(&mut layer, "scope-reject-file", "x")?;

    assert!(layer.xfail(&["reject"]));
    assert!(layer.last_stderr.contains(
        "reject requires at least one pattern; use `devlayer reject /` to reject every change in the layer"
    ));

    // Nothing was rejected
    layer.run(&["status", "/"])?;
    assert!(layer.last_stdout.contains("scope-reject-file"));
    Ok(())
}

/// Without a pattern, accept and reject say how to accept or reject
/// everything, even from a directory that has no layer to default to
#[rstest]
fn ai_test_pattern_hint_without_layer(mut layer: LayerManager) -> Result<()> {
    let dir = std::env::temp_dir().join(format!("devlayer-coverage-{}", rid()));
    std::fs::create_dir_all(&dir)?;
    // No --join, so --last would apply
    layer.no_default_options = true;
    layer.cwd = Some(dir.clone());
    for action in ["accept", "reject"] {
        assert!(layer.xfail(&[action]));
        assert!(
            layer
                .last_stderr
                .contains(&format!("use `devlayer {} /`", action)),
            "{}",
            layer.last_stderr
        );
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}
