mod fixtures;

use std::{path::PathBuf, process::Command};

use anyhow::Result;
use fixtures::*;
use rstest::*;

#[rstest]
fn test_status(mut layer: LayerManager) -> Result<()> {
    let filename = layer.test_filename("file");
    let removed = layer.test_filename("removed");
    let moved = layer.test_filename("moved");
    let moved_2 = layer.test_filename("moved_2");

    std::fs::write(&removed, "test")?;
    std::fs::create_dir(&moved)?;

    layer.run(&["touch", &filename])?;
    layer.run(&["mv", &moved, &moved_2])?;
    layer.run(&["rm", &removed])?;

    layer.run(&["status", "/"])?;
    assert!(layer.last_stdout.contains("Matching changes"));
    assert!(layer.last_stdout.contains(&filename));
    assert!(!layer.last_stdout.contains("No changes in"));
    println!("{}", layer.last_stdout);
    assert!(
        !layer
            .last_stdout
            .contains("external or non-matching changes")
    );
    assert!(layer.last_stdout.contains(&format!("> {}", &moved)));
    assert!(layer.last_stdout.contains(&format!("- {}", &removed)));

    layer.run(&["status", "/", &filename])?;
    assert!(layer.last_stdout.contains("Matching changes"));
    assert!(layer.last_stdout.contains(&filename));
    assert!(!layer.last_stdout.contains("No changes in"));
    assert!(
        !layer
            .last_stdout
            .contains("external or non-matching changes")
    );

    layer.run(&["status", "/", "foobar"])?;
    assert!(layer.last_stdout.contains("Matching changes"));
    assert!(layer.last_stdout.contains(&filename));
    assert!(
        !layer
            .last_stdout
            .contains("external or non-matching changes")
    );

    Ok(())
}

#[rstest]
fn test_status_outside(mut layer: LayerManager) -> Result<()> {
    let filename = "/tmp/test";
    layer.run(&["touch", filename])?;
    layer.run(&["status", "."])?;
    println!("{}", layer.last_stdout);
    assert!(
        layer
            .last_stdout
            .contains("external or non-matching changes")
    );
    layer.run(&["status", "/", filename])?;
    println!("{}", layer.last_stdout);
    assert!(layer.last_stdout.contains("Matching changes"));
    assert!(layer.last_stdout.contains(filename));
    Ok(())
}

#[rstest]
fn test_status_no_matching_changes(mut layer: LayerManager) -> Result<()> {
    let filename = "/tmp/test";
    layer.run(&["touch", filename])?;
    layer.run(&["status", "not-the-file-name"])?;
    assert!(layer.last_stdout.contains("No matching changes"));
    Ok(())
}

#[rstest]
fn test_status_changed_attributes(mut layer: LayerManager) -> Result<()> {
    let mode_file = layer.test_filename("mode");
    let uid_file = layer.test_filename("uid");
    let gid_file = layer.test_filename("gid");
    std::fs::write(&mode_file, "test")?;
    std::fs::write(&uid_file, "test")?;
    std::fs::write(&gid_file, "test")?;

    layer.run(&["touch", mode_file.as_str()])?;
    layer.run(&["touch", uid_file.as_str()])?;
    layer.run(&["touch", gid_file.as_str()])?;

    assert!(layer.pass(&["status", "."]));
    assert!(!layer.last_stderr.contains(&mode_file));
    assert!(!layer.last_stderr.contains(&uid_file));
    assert!(!layer.last_stderr.contains(&gid_file));

    layer.run(&["chmod", "777", mode_file.as_str()])?;
    assert!(layer.pass(&["status", "."]));
    assert!(layer.last_stderr.contains(&mode_file));
    assert!(!layer.last_stderr.contains(&uid_file));
    assert!(!layer.last_stderr.contains(&gid_file));

    Command::new("sudo")
        .args(["chown", "2", uid_file.as_str()])
        .output()?;
    Command::new("sudo")
        .args(["chgrp", "2", gid_file.as_str()])
        .output()?;

    layer.run(&["chmod", "777", uid_file.as_str()])?;
    assert!(layer.pass(&["status", "."]));
    assert!(layer.last_stderr.contains(&mode_file));
    assert!(layer.last_stderr.contains(&uid_file));
    assert!(layer.last_stderr.contains(&gid_file));

    Ok(())
}

#[rstest]
fn test_external_changes(mut layer: LayerManager) -> Result<()> {
    let filename = "/tmp/test-external-changes-pre";
    std::fs::write(filename, "test")?;
    layer.run(&["touch", "/tmp/test-external-changes-post"])?;
    layer.run(&[
        "bash",
        "-c",
        "echo 'test2' > /tmp/test-external-changes-pre",
    ])?;
    layer.run(&["status", "/"])?;
    assert!(layer.last_stdout.contains("Matching changes"));
    assert!(layer.last_stdout.contains("/tmp/test-external-changes-pre"));
    assert!(
        layer
            .last_stdout
            .contains("/tmp/test-external-changes-post")
    );

    std::fs::remove_file(filename)?;

    Ok(())
}

#[rstest]
fn test_status_gitignore(mut layer: LayerManager) -> Result<()> {
    let base = PathBuf::from(&layer.test_filename("test"));
    std::fs::create_dir_all(base.to_str().unwrap())?;
    let gitignore_path = base.join(".gitignore");
    std::fs::write(
        gitignore_path.to_str().unwrap(),
        "
# Allow everything that was previously blocked by the project's gitignore
!*

# Things we want to test
empty
dir1
suffix/
two
/anchored
space-at-end\\ 

!**/present*
",
    )?;

    layer.run(&["mkdir", base.join("empty").to_str().unwrap()])?;
    layer.run(&["mkdir", base.join("dir1").to_str().unwrap()])?;
    layer.run(&["mkdir", base.join("suffix").to_str().unwrap()])?;
    layer.run(&["mkdir", "-p", base.join("two/.part").to_str().unwrap()])?;
    layer.run(&["mkdir", base.join("anchored").to_str().unwrap()])?;
    layer.run(&["touch", base.join("space-at-end").to_str().unwrap()])?;
    layer.run(&["touch", base.join("space-at-end ").to_str().unwrap()])?;

    layer.run(&["touch", base.join("dir1/file1").to_str().unwrap()])?;
    layer.run(&["touch", base.join("suffix/file2").to_str().unwrap()])?;
    layer.run(&["touch", base.join("two/.part/file3").to_str().unwrap()])?;
    layer.run(&["touch", base.join("anchored/file4").to_str().unwrap()])?;

    layer.run(&["touch", base.join("dir1/present1").to_str().unwrap()])?;
    layer.run(&["touch", base.join("suffix/present2").to_str().unwrap()])?;
    layer.run(&["touch", base.join("two/.part/present3").to_str().unwrap()])?;
    layer.run(&["touch", base.join("anchored/present4").to_str().unwrap()])?;

    layer.set_ignored(false);
    layer.run(&["status", base.to_str().unwrap()])?;

    println!("{}", layer.last_stderr);
    println!("{}", layer.last_stdout);

    assert!(!layer.last_stdout.contains("space-at-end "));
    assert!(!layer.last_stdout.contains("empty"));
    assert!(!layer.last_stdout.contains("file1"));
    assert!(!layer.last_stdout.contains("file2"));
    assert!(!layer.last_stdout.contains("file3"));
    assert!(!layer.last_stdout.contains("file4"));

    assert!(layer.last_stdout.contains("space-at-end"));
    assert!(layer.last_stdout.contains("present1"));
    assert!(layer.last_stdout.contains("present2"));
    assert!(layer.last_stdout.contains("present3"));
    assert!(layer.last_stdout.contains("present4"));

    Ok(())
}
