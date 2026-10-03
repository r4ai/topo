//! Directory-anchored access to workspace files. Links cannot escape the root,
//! and replacement never truncates an existing symlink or hard link.

use std::io::{self, Write};
use std::path::Path;

pub use cap_fs_ext::DirExt;
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use cap_tempfile::TempFile;

/// Opens the selected directory without following a link at its final component.
/// Ancestor links (including macOS `/tmp`) remain usable.
pub fn open_dir(path: &Path) -> io::Result<Dir> {
    let Some(name) = path.file_name() else {
        return Dir::open_ambient_dir(path, ambient_authority());
    };
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Dir::open_ambient_dir(parent, ambient_authority())?.open_dir_nofollow(name)
}

pub fn read(root: &Path, name: &str) -> io::Result<String> {
    open_dir(root)?.read_to_string(name)
}

/// Replaces a file using an exclusively created temporary file in the same
/// open directory. The handle remains anchored if directory entries change.
pub fn replace(dir: &Dir, name: &str, text: &str, private: bool) -> io::Result<()> {
    if private {
        let (temporary_name, mut file) = private_file(dir)?;
        let result = file.write_all(text.as_bytes()).and_then(|_| dir.rename(&temporary_name, dir, name));
        drop(file);
        let _ = dir.remove_file(&temporary_name);
        return result;
    }
    let mut temp = TempFile::new(dir)?;
    temp.write_all(text.as_bytes())?;
    temp.replace(name)
}

fn private_file(dir: &Dir) -> io::Result<(String, cap_std::fs::File)> {
    let mut options = cap_std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    for _ in 0..16 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
        let random = u128::from_le_bytes(bytes);
        let name = format!(".topo-private-{random:032x}.tmp");
        match dir.open_with(&name, &options) {
            Ok(file) => return Ok((name, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "could not create a private temporary file"))
}

pub fn write(root: &Path, name: &str, text: &str, private: bool) -> io::Result<()> {
    replace(&open_dir(root)?, name, text, private)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn private_temporary_files_are_owner_only_from_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = open_dir(tmp.path()).unwrap();
        let (name, file) = private_file(&dir).unwrap();
        assert_eq!(std::fs::metadata(tmp.path().join(&name)).unwrap().permissions().mode() & 0o777, 0o600);
        drop(file);
        dir.remove_file(name).unwrap();
    }

    #[test]
    fn directory_handle_stays_anchored_when_its_name_is_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let selected = tmp.path().join("selected");
        let outside = tmp.path().join("outside");
        std::fs::create_dir(&selected).unwrap();
        std::fs::create_dir(&outside).unwrap();
        let dir = open_dir(&selected).unwrap();
        std::fs::rename(&selected, tmp.path().join("original")).unwrap();
        symlink(&outside, &selected).unwrap();
        replace(&dir, "secret", "value", true).unwrap();
        assert!(!outside.join("secret").exists());
        let saved = tmp.path().join("original/secret");
        assert_eq!(std::fs::read_to_string(&saved).unwrap(), "value");
        assert_eq!(std::fs::metadata(&saved).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn private_replacement_repairs_permissions_and_breaks_hard_links() {
        let tmp = tempfile::tempdir().unwrap();
        let victim = tmp.path().join("victim");
        std::fs::write(&victim, "old").unwrap();
        std::fs::set_permissions(&victim, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::hard_link(&victim, tmp.path().join("credentials.toml")).unwrap();
        write(tmp.path(), "credentials.toml", "new", true).unwrap();
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "old");
        assert_eq!(std::fs::metadata(tmp.path().join("credentials.toml")).unwrap().permissions().mode() & 0o777, 0o600);
    }
}
