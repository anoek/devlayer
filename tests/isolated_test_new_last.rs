mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_new_last() -> Result<()> {
    let mut layer1 = LayerManager::new();
    let mut layer2 = LayerManager::new();
    let mut layer3 = LayerManager::new();

    layer1.no_default_options = true;
    layer2.no_default_options = true;
    layer3.no_default_options = true;

    assert!(layer1.pass(&["--new", "-v", "ls"]));
    assert!(layer1.pass(&["--last", "config", "name"]));
    let layer1_name = layer1.last_stdout.trim();
    layer1.name = layer1_name.to_string();

    assert!(layer2.pass(&["--new", "-v", "ls"]));
    assert!(layer2.pass(&["--last", "config", "name"]));
    let layer2_name = layer2.last_stdout.trim();
    layer2.name = layer2_name.to_string();

    assert!(layer3.pass(&["--new", "-v", "ls"]));
    assert!(layer3.pass(&["--last", "config", "name"]));
    let layer3_name = layer3.last_stdout.to_string();
    layer3.name = layer3_name.trim().to_string();

    assert!(layer3.pass(&["--last", "-v", "ls"]));
    assert!(layer3.pass(&["--last", "config", "name"]));
    let layer3_last_name = layer3.last_stdout.trim().to_string();

    assert_ne!(layer1_name, layer2_name);
    assert_ne!(layer1_name, layer3_name);
    assert_ne!(layer2_name, layer3_name);
    assert_eq!(layer3_name.trim(), layer3_last_name);

    Ok(())
}
