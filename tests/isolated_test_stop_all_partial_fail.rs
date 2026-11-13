mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{process::Command, thread::sleep, time::Duration};

/* This test needs to be the only test in this file since it
 * kills all running layers, which would interfere with other
 * tests that were running in parallel. */

#[rstest]
#[ignore = "Problematic on dev machines with running layers"]
fn test_stop_all_partial_fail() -> Result<()> {
    let mut bad_layer = layer();
    bad_layer.run(&["true"])?;
    bad_layer.run(&["config", "layer_dir"])?;
    let layer_dir = String::from(bad_layer.last_stdout.trim());
    let lock_file = format!("{}.lock", layer_dir);
    println!("lock_file: {}", lock_file);
    Command::new("sudo")
        .args(["rm", "-f", lock_file.as_str()])
        .output()?;
    std::fs::create_dir_all(&lock_file)?;
    // lock file is now a directory, should fail
    assert!(bad_layer.xfail(&["true"]));
    assert!(bad_layer.all_stderr.contains("Failed to open lock"));

    /* Despite bad state that the above "devlayer" is in, the following is expected to work. */
    let mut layer1 = layer();
    let mut layer2 = layer();
    let mut c1 = layer1.run_in_background(&["sleep", "9897135"])?;
    let mut c2 = layer2.run_in_background(&["sleep", "9897135"])?;

    // ensure they start up
    for _ in 0..100 {
        sleep(Duration::from_millis(10));
        let ps = Command::new("ps").arg("aux").output()?;
        let ps_output = String::from_utf8(ps.stdout)?;
        let lines: Vec<&str> = ps_output
            .lines()
            .filter(|line| {
                line.contains("sleep 9897135") && !line.contains("--name")
            })
            .collect();
        if lines.len() == 2 {
            break;
        }
    }

    // ensure they're actually there
    let ps = Command::new("ps").arg("aux").output()?;
    let ps_output = String::from_utf8(ps.stdout)?;
    let lines: Vec<&str> = ps_output
        .lines()
        .filter(|line| {
            line.contains("sleep 9897135") && !line.contains("--name")
        })
        .collect();
    assert!(lines.len() == 2);

    assert!(layer1.pass(&["stop", "--all"]));
    c1.wait()?;

    let ps = Command::new("ps").arg("aux").output()?;
    let ps_output = String::from_utf8(ps.stdout)?;
    let lines: Vec<&str> = ps_output
        .lines()
        .filter(|line| {
            line.contains("sleep 9897135") && !line.contains("--name")
        })
        .collect();
    assert!(lines.is_empty());

    c2.wait()?; // if this doesn't hang, then the layer was killed!

    /* Restore the bad layer to a working state so our cleanup works. */
    // remove directory
    std::fs::remove_dir(&lock_file)?;
    // lock file is now a file, should succeed
    assert!(bad_layer.pass(&["true"]));
    Ok(())
}
