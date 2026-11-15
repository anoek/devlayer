use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

/// Writes `contents` to `path`, replacing what's there, but fails rather
/// than follow a symlink at `path`. For files we write as root in
/// directories the user owns.
pub fn write_no_follow(
    path: &Path,
    contents: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?
        .write_all(contents.as_ref())
}
