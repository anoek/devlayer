mod fixtures;

use std::path::{Path, PathBuf};

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_list() -> Result<()> {
    let mut layer1 = layer();
    let mut layer2 = layer();

    layer1.run(&["true"])?;
    layer2.run(&["true"])?;

    let name1 = layer1.name.clone();
    let name2 = layer2.name.clone();

    layer1.run(&["list"])?;
    let running = layer1.last_stdout.clone();
    println!("looking for {} in {}", name1, running);
    assert!(running.contains(&name1));
    assert!(running.contains(&name2));

    layer1.run(&["list", &name1])?;
    let running = layer1.last_stdout.clone();
    println!("looking for {} and {} in {}", name1, name2, running);
    assert!(running.contains(&name1));
    assert!(!running.contains(&name2));

    layer1.run(&["list", &name1, &name2])?;
    let running = layer1.last_stdout.clone();
    assert!(running.contains(&name1));
    assert!(running.contains(&name2));

    layer1.run(&["list", "non existent"])?;
    let running = layer1.last_stdout.clone();
    assert!(!running.contains(&name1));
    assert!(!running.contains(&name2));
    Ok(())
}

#[rstest]
fn test_list_ignore_bad_pid_file(mut layer: LayerManager) -> Result<()> {
    layer.run(&["config", "layer_dir"])?;
    let mut t = PathBuf::from(layer.last_stdout.clone());
    println!("t: {}", t.display());
    t.pop();
    println!("t: {}", t.display());
    let bad_layer_base =
        format!("{}/{}", t.to_string_lossy(), "coverage_test_bad_pid_file");
    println!("bad_layer_base: {}", bad_layer_base);

    let pid_file_name = format!("{}.pid", bad_layer_base);
    let lock_file_name = format!("{}.lock", bad_layer_base);

    std::fs::write(&lock_file_name, "1")?;
    std::fs::write(&pid_file_name, "not a number")?;

    println!("pid_file_name: {}", pid_file_name);
    let pid_file = std::fs::File::open(&pid_file_name)?;
    let pid_file_content = std::io::read_to_string(pid_file)?;
    println!("pid_file_content: {}", pid_file_content);
    assert!(pid_file_content.parse::<i32>().is_err());

    assert!(layer.run(&["list"]).is_ok());
    // still expected to be listed
    assert!(layer.all_stdout.contains("coverage_test_bad_pid_file"));
    // but listing doesn't hold the layer lock, so it leaves the pid file
    // alone, as it might be one that's being written
    assert!(Path::new(&pid_file_name).exists());

    std::fs::remove_file(&lock_file_name)?;

    Ok(())
}

#[rstest]
fn test_list_stale_pid_file(mut layer: LayerManager) -> Result<()> {
    layer.run(&["config", "layer_dir"])?;
    let mut t = PathBuf::from(layer.last_stdout.clone());
    t.pop();
    let stale_layer_base =
        format!("{}/{}", t.to_string_lossy(), layer.name.clone());

    let pid_file_name = format!("{}.pid", stale_layer_base);

    layer.run(&["true"])?;
    let pid = std::fs::read_to_string(&pid_file_name)?;
    layer.run(&["stop"])?;

    // stop cleans up our pid, but let's restore it so we have a stale pid
    // file that shouldn't conflict with any existing processes on the system
    // at this point.
    std::fs::write(&pid_file_name, pid)?;

    println!("pid_file_name: {}", pid_file_name);
    let pid_file = std::fs::File::open(&pid_file_name)?;
    let pid_file_content = std::io::read_to_string(pid_file)?;
    println!("pid_file_content: {}", pid_file_content);
    let pid = pid_file_content
        .split_whitespace()
        .next()
        .unwrap_or_default();
    assert!(pid.parse::<i32>().is_ok());

    assert!(layer.run(&["list", &layer.name.clone()]).is_ok());
    eprintln!(">>>> {} <<<<", layer.last_stdout);
    eprintln!(">>>> {} <<<<", layer.last_stderr);
    // still expected to be listed
    assert!(layer.all_stdout.contains("Stopped"));
    assert!(layer.all_stdout.contains(&layer.name));

    Ok(())
}
