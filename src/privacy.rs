//! Account storage stays private even with a permissive umask or an old install.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

fn regular_file(file: &File) -> io::Result<()> {
    let meta = file.metadata()?;
    if !meta.is_file() || meta.nlink() != 1 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "account data must be a regular file without hard links"));
    }
    Ok(())
}

pub(crate) fn prepare_dir(path: &Path) -> io::Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(path)?;
    let dir = OpenOptions::new().read(true).custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW).open(path)?;
    dir.set_permissions(fs::Permissions::from_mode(0o700))?;
    // Close access to legacy databases, sidecars, drafts and rotated logs too.
    // Do not follow links or change permissions of files outside this directory.
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            continue;
        }
        let file = match OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(entry.path()) {
            Ok(file) => file,
            // SQLite may remove a journal while another instance is running.
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        regular_file(&file)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub(crate) fn create_file(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    regular_file(&file)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.set_len(0)?;
    Ok(file)
}

pub(crate) fn write_file(path: &Path, data: impl AsRef<[u8]>) -> io::Result<()> {
    create_file(path)?.write_all(data.as_ref())
}
