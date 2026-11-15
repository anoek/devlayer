mod fixtures;

use anyhow::Result;
use fixtures::*;
use rstest::*;
use std::ffi::CStr;
use std::path::Path;

#[rstest]
fn test_fail_to_read_mounts(mut layer: LayerManager) -> Result<()> {
    assert!(layer.exfail(&["env"], "TEST_SYSTEM_MOUNTS_NULL", "1"));
    layer.hold()?;
    assert!(layer.exfail(&["stop"], "TEST_SYSTEM_MOUNTS_NULL", "1"));
    assert!(layer.pass(&["stop"]));
    Ok(())
}

#[rstest]
fn test_no_mounts_found(mut layer: LayerManager) -> Result<()> {
    assert!(layer.exfail(&["env"], "TEST_NO_MOUNTS_FOUND", "1"));
    layer.hold()?;
    assert!(layer.epass(&["stop"], "TEST_NO_MOUNTS_FOUND", "1"));
    assert!(layer.last_stderr.contains("No mounts found for layer"));
    Ok(())
}

#[rstest]
fn test_unmount_race(mut layer: LayerManager) -> Result<()> {
    layer.hold()?;
    assert!(layer.epass(&["stop"], "TEST_UNMOUNT_RACE", "1"));
    assert!(layer.last_stderr.contains("Failed to unmount"));
    Ok(())
}

#[rstest]
fn test_kill_not_running(mut layer: LayerManager) -> Result<()> {
    assert!(layer.pass(&["stop"]));
    assert!(layer.last_stderr.contains("doesn't appear to be running"));

    layer.hold()?;
    assert!(layer.epass(&["stop"], "TEST_STOP_RACE", "1"));
    assert!(layer.last_stderr.contains("already gone"));

    layer.hold()?;
    assert!(layer.epass(&["stop"], "TEST_STOP_RACE2", "1"));
    assert!(layer.last_stderr.contains("Failed to kill process"));

    layer.hold()?;
    assert!(layer.epass(&["stop"], "TEST_STOP_RACE3", "1"));
    Ok(())
}

#[rstest]
fn test_kill_unmounts(mut layer: LayerManager) -> Result<()> {
    layer.hold()?;

    let mounts = get_mounts(&layer.dir()?);
    assert!(!mounts.is_empty());

    assert!(layer.pass(&["stop"]));
    let mounts = get_mounts(&layer.dir()?);
    assert!(mounts.is_empty());

    Ok(())
}

/* Utility function for getting mounts */
pub fn get_mounts(base: &Path) -> Vec<String> {
    let mut mounts = Vec::new();

    let system_mounts =
        unsafe { libc::setmntent(c"/proc/mounts".as_ptr(), c"r".as_ptr()) };

    assert!(!system_mounts.is_null());

    loop {
        let mnt = unsafe { libc::getmntent(system_mounts) };
        if mnt.is_null() {
            break;
        }

        let mnt_dir = String::from(unsafe {
            CStr::from_ptr((*mnt).mnt_dir).to_string_lossy()
        });

        if Path::new(&mnt_dir).starts_with(base) {
            mounts.push(mnt_dir);
        }
    }

    unsafe { libc::endmntent(system_mounts) };

    mounts.sort_by(|a, b| b.cmp(a));

    mounts
}
