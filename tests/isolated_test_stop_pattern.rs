mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::{process::Command, thread::sleep, time::Duration};

/* This test needs to be the only test in this file since it
 * kills all running layers, which would interfere with other
 * tests that were running in parallel. */

#[rstest]
fn test_stop_pattern() -> Result<()> {
    let mut layer1 = layer();
    let mut layer2 = layer();
    let mut c1 = layer1.run_in_background(&["sleep", "9897135"])?;
    let mut c2 = layer2.run_in_background(&["sleep", "9897135"])?;

    // ensure they start up
    for _ in 0..1000 {
        sleep(Duration::from_millis(10));
        let ps = Command::new("ps").arg("aux").output()?;
        let ps_output = String::from_utf8(ps.stdout)?;
        let lines: Vec<&str> = ps_output
            .lines()
            .filter(|line| {
                line.contains("sleep 9897135") && !line.contains("--join")
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
            line.contains("sleep 9897135") && !line.contains("--join")
        })
        .collect();
    assert!(lines.len() == 2);

    assert!(layer1.pass(&["stop", layer2.name.as_str()]));
    c2.wait()?;

    let ps = Command::new("ps").arg("aux").output()?;
    let ps_output = String::from_utf8(ps.stdout)?;
    let lines: Vec<&str> = ps_output
        .lines()
        .filter(|line| {
            line.contains("sleep 9897135") && !line.contains("--join")
        })
        .collect();
    assert!(lines.len() == 1);

    layer1.run(&["stop"])?;

    c1.wait()?; // if this doesn't hang, then the layer was killed!
    Ok(())
}
