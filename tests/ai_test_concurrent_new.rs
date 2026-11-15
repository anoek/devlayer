mod fixtures;

use anyhow::Result;
use fixtures::*;
use std::collections::BTreeSet;
use std::process::{Command, Stdio};

/// Commands launched at the same moment, each in a new layer, generate the
/// same timestamp name, yet each still gets a layer of its own
#[test]
fn ai_test_concurrent_new_launches_get_own_layers() -> Result<()> {
    let bin = get_devlayer_bin();
    let launches = (0..8)
        .map(|_| {
            as_root(Command::new(&bin).args([
                "--no-config",
                "--no-bind-cwd",
                "--new",
                "sh",
                "-c",
                "echo $DEVLAYER",
            ]))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let outputs = launches
        .into_iter()
        .map(|launch| launch.wait_with_output())
        .collect::<std::io::Result<Vec<_>>>()?;
    let names: Vec<String> = outputs
        .iter()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        })
        .collect();

    // Deletes the layers when dropped
    let _layers: Vec<LayerManager> = names
        .iter()
        .filter(|name| !name.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|name| {
            let mut layer = LayerManager::new();
            layer.name = name.clone();
            layer
        })
        .collect();

    for output in &outputs {
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        names.iter().collect::<BTreeSet<_>>().len(),
        names.len(),
        "launches shared layers: {:?}",
        names
    );
    Ok(())
}
