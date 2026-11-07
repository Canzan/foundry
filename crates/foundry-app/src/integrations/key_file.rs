//! The instance key file (ADR-IIS-002): without an operator instance key,
//! Foundry reads `instance.key` in `FOUNDRY_DATA_DIR`, or makes it once.
//!
//! The only code that knows the data directory or touches the file
//! (check-arch `integrations-key-file-seam`). Read at boot only: a reload
//! reuses the key boot held, so a file deleted while the server runs is never
//! made again.
//!
//! Making the file: a temp file `instance.key.tmp.{random}` opened
//! `O_CREAT|O_EXCL|O_WRONLY` with mode 0600 at open, the key written and
//! `fsync`ed, then `link(2)`ed to `instance.key` (`EEXIST`: another server
//! won), the directory `fsync`ed, the temp file removed and whichever file
//! exists read back. An existing `instance.key` is never overwritten, chmod-ed
//! or removed. No key, and no file content, is ever logged.

use super::keys::{InstanceKeySource, KeyFileRead};
use foundry_auth::secret_box::{SecretsKey, INSTANCE_KEY_SETTING};
use secrecy::{ExposeSecret as _, SecretString};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

/// The data directory's setting.
pub const DATA_DIR: &str = "FOUNDRY_DATA_DIR";
/// Its default: a standalone deploy's volume (the image creates it).
pub const DEFAULT_DATA_DIR: &str = "/var/lib/foundry";

const FILE_NAME: &str = "instance.key";
const TEMP_PREFIX: &str = "instance.key.tmp.";
/// The only bits the file may carry (owner read and write).
const PRIVATE_MODE: u32 = 0o600;
/// A key file is one line of 43 characters; anything far larger is not one.
const MAX_FILE_BYTES: u64 = 1024;

/// The key file's path in `data_dir`, as the page names it.
pub fn key_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

/// The instance key file in one data directory: the [`InstanceKeySource`]
/// the shared boot is given.
pub struct InstanceKeyFile {
    dir: PathBuf,
}

impl InstanceKeyFile {
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The key file of `settings`' data directory.
    pub fn of(settings: &super::config::IntegrationSettings) -> Self {
        Self::in_dir(settings.data_dir.clone())
    }

    fn path(&self) -> PathBuf {
        key_path(&self.dir)
    }

    /// The probe (ADR-IIS-002 r2, r6): leftover temp files removed, then the
    /// existing file read, or a new one made, linked, read back and checked
    /// before its key is used.
    fn probe(&self) -> KeyFileRead {
        self.remove_leftovers();
        match std::fs::symlink_metadata(self.path()) {
            Ok(_) => self.read_existing(),
            // No file there; a data directory that is no directory cannot
            // take one, which the attempt reports (I-27).
            Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
                self.create()
            }
            Err(err) => {
                tracing::warn!(
                    event = "integrations.key_file.unreadable",
                    error_kind = ?err.kind(),
                    "the instance key file cannot be read"
                );
                KeyFileRead::Unreadable
            }
        }
    }

    /// Temp files a crashed start left behind; `instance.key` itself is never
    /// touched. A directory that cannot be listed has none to remove.
    fn remove_leftovers(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// A file that was already there: used when it parses, even with wider
    /// bits (warned, never chmod-ed); never rewritten when it does not.
    fn read_existing(&self) -> KeyFileRead {
        match read_key(&self.path()) {
            Ok((key, mode)) => {
                if mode & 0o077 != 0 {
                    tracing::warn!(
                        event = "integrations.key_file.wide_mode",
                        mode = format!("{mode:o}"),
                        "the instance key file is readable beyond its owner; it is used and left as it is"
                    );
                }
                KeyFileRead::Held(Box::new(key))
            }
            Err(problem) => {
                tracing::warn!(
                    event = "integrations.key_file.unreadable",
                    problem,
                    "the instance key file cannot be read; it is kept as it is"
                );
                KeyFileRead::Unreadable
            }
        }
    }

    /// Make the file once. A directory that cannot take it is `Unwritable`
    /// (I-27); losing the race to another server reads the winner's file.
    fn create(&self) -> KeyFileRead {
        let temp = self
            .dir
            .join(format!("{TEMP_PREFIX}{}", uuid::Uuid::new_v4().simple()));
        let made = write_temp(&temp).and_then(|uid| {
            let linked = std::fs::hard_link(&temp, self.path());
            let _ = std::fs::remove_file(&temp);
            match linked {
                Ok(()) => {
                    sync_dir(&self.dir);
                    Ok(Some(uid))
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(None),
                Err(err) => Err(err),
            }
        });
        match made {
            Ok(Some(uid)) => self.read_created(uid),
            Ok(None) => self.read_existing(),
            Err(err) => {
                let _ = std::fs::remove_file(&temp);
                tracing::warn!(
                    event = "integrations.key_file.unwritable",
                    error_kind = ?err.kind(),
                    "the instance key file cannot be made"
                );
                KeyFileRead::Unwritable
            }
        }
    }

    /// The file this server just made: it must be a regular file of mode
    /// 0600 owned by `uid` that parses, else the directory cannot be trusted
    /// to hold a key (`Unwritable`).
    fn read_created(&self, uid: u32) -> KeyFileRead {
        let owned = std::fs::symlink_metadata(self.path()).is_ok_and(|meta| meta.uid() == uid);
        match read_key(&self.path()) {
            Ok((key, mode)) if owned && mode == PRIVATE_MODE => KeyFileRead::Held(Box::new(key)),
            _ => {
                tracing::warn!(
                    event = "integrations.key_file.unwritable",
                    "the instance key file made here failed its check"
                );
                KeyFileRead::Unwritable
            }
        }
    }
}

impl InstanceKeySource for InstanceKeyFile {
    fn key_file(&self) -> KeyFileRead {
        self.probe()
    }
}

/// A new key written to `temp`, created 0600 at open and `fsync`ed; the
/// temp file's owner.
fn write_temp(temp: &Path) -> std::io::Result<u32> {
    let generated = SecretsKey::generate().map_err(|_| std::io::Error::other("random source"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(PRIVATE_MODE)
        .open(temp)?;
    file.write_all(format!("{}\n", generated.text.expose_secret()).as_bytes())?;
    file.sync_all()?;
    Ok(file.metadata()?.uid())
}

/// `fsync` the directory so the link survives a crash. A filesystem that
/// cannot is no reason to refuse the key already linked.
fn sync_dir(dir: &Path) {
    if let Err(err) = File::open(dir).and_then(|d| d.sync_all()) {
        tracing::warn!(
            event = "integrations.key_file.dir_sync",
            error_kind = ?err.kind(),
            "the data directory could not be synced"
        );
    }
}

/// The key in the regular file at `path`, and the file's permission bits;
/// the problem otherwise (never the content).
fn read_key(path: &Path) -> Result<(SecretsKey, u32), &'static str> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| "no metadata")?;
    if !meta.file_type().is_file() {
        return Err("not a regular file");
    }
    let mut text = String::new();
    File::open(path)
        .map_err(|_| "cannot open")?
        .take(MAX_FILE_BYTES)
        .read_to_string(&mut text)
        .map_err(|_| "cannot read")?;
    let line = SecretString::from(text.trim().to_string());
    let key = SecretsKey::parse(&line, INSTANCE_KEY_SETTING).map_err(|_| "does not parse")?;
    Ok((key, meta.permissions().mode() & 0o777))
}

#[cfg(test)]
mod tests {
    //! Example-based fault injection on a real temp directory (filesystem
    //! effects: exempt from proptest).
    use super::*;

    /// A temp directory of this test's own, removed on drop.
    struct Dir(PathBuf);

    impl Dir {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "foundry-key-file-{}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::create_dir(&dir).expect("make the temp directory");
            Self(dir)
        }
        fn file(&self) -> PathBuf {
            key_path(&self.0)
        }
        fn plant(&self, bytes: &[u8], mode: u32) {
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(mode)
                .open(self.file())
                .expect("plant the key file");
            f.write_all(bytes).expect("write the planted file");
            std::fs::set_permissions(self.file(), std::fs::Permissions::from_mode(mode))
                .expect("set the planted mode");
        }
        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = std::fs::read_dir(&self.0)
                .expect("list the directory")
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn held_id(read: KeyFileRead) -> String {
        match read {
            KeyFileRead::Held(key) => key.key_id().to_string(),
            KeyFileRead::Unwritable => panic!("expected a held key, got Unwritable"),
            KeyFileRead::Unreadable => panic!("expected a held key, got Unreadable"),
        }
    }

    fn state(read: &KeyFileRead) -> &'static str {
        match read {
            KeyFileRead::Held(_) => "held",
            KeyFileRead::Unwritable => "unwritable",
            KeyFileRead::Unreadable => "unreadable",
        }
    }

    #[test]
    fn an_absent_file_is_made_once_private_and_read_back_the_same() {
        let dir = Dir::new();
        let first = held_id(InstanceKeyFile::in_dir(&dir.0).key_file());
        let meta = std::fs::metadata(dir.file()).expect("the file was made");
        assert_eq!(meta.permissions().mode() & 0o777, PRIVATE_MODE);
        let bytes = std::fs::read(dir.file()).expect("read the file");
        let second = held_id(InstanceKeyFile::in_dir(&dir.0).key_file());
        assert_eq!(first, second, "a second boot reads the same key");
        assert_eq!(std::fs::read(dir.file()).unwrap(), bytes, "never rewritten");
        assert_eq!(
            dir.names(),
            vec![FILE_NAME.to_string()],
            "no temp file left"
        );
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text.trim().len(), 43, "one line of base64url of 32 bytes");
    }

    #[test]
    fn two_servers_racing_on_one_directory_hold_one_key() {
        let dir = Dir::new();
        let ids: Vec<String> = std::thread::scope(|s| {
            let racers: Vec<_> = (0..8)
                .map(|_| s.spawn(|| held_id(InstanceKeyFile::in_dir(&dir.0).key_file())))
                .collect();
            racers.into_iter().map(|r| r.join().unwrap()).collect()
        });
        assert!(ids.iter().all(|id| *id == ids[0]), "{ids:?}");
        assert_eq!(dir.names(), vec![FILE_NAME.to_string()]);
    }

    #[test]
    fn a_leftover_temp_file_is_removed_and_the_file_made() {
        let dir = Dir::new();
        let leftover = dir.0.join(format!("{TEMP_PREFIX}crashed"));
        std::fs::write(&leftover, b"partial").unwrap();
        held_id(InstanceKeyFile::in_dir(&dir.0).key_file());
        assert_eq!(dir.names(), vec![FILE_NAME.to_string()]);
    }

    #[test]
    fn a_directory_that_cannot_take_the_file_is_unwritable() {
        // A path under a regular file can never be a directory, whoever runs
        // the test; a read-only directory is one only for a non-root user.
        let dir = Dir::new();
        let blocker = dir.0.join("not-a-directory");
        std::fs::write(&blocker, b"a file\n").unwrap();
        let read = InstanceKeyFile::in_dir(&blocker).key_file();
        assert_eq!(state(&read), "unwritable");

        let read_only = Dir::new();
        std::fs::set_permissions(&read_only.0, std::fs::Permissions::from_mode(0o500)).unwrap();
        let writable_anyway = File::create(read_only.0.join("probe")).is_ok();
        let read = InstanceKeyFile::in_dir(&read_only.0).key_file();
        let expected = if writable_anyway {
            "held"
        } else {
            "unwritable"
        };
        assert_eq!(state(&read), expected, "a read-only directory");
        if !writable_anyway {
            assert!(read_only.names().is_empty(), "nothing left behind");
        }

        let missing = dir.0.join("never-made");
        assert_eq!(
            state(&InstanceKeyFile::in_dir(&missing).key_file()),
            "unwritable"
        );
    }

    #[test]
    fn a_file_that_cannot_be_read_is_unreadable_and_never_changed() {
        let generated = SecretsKey::generate().unwrap();
        let key = generated.text.expose_secret().to_string();
        for (fault, bytes) in [
            ("a corrupt file", b"this is not an instance key\n".to_vec()),
            ("a short read", format!("{}\n", &key[..20]).into_bytes()),
            ("an empty file", Vec::new()),
        ] {
            let dir = Dir::new();
            dir.plant(&bytes, PRIVATE_MODE);
            let read = InstanceKeyFile::in_dir(&dir.0).key_file();
            assert_eq!(state(&read), "unreadable", "{fault}");
            assert_eq!(
                std::fs::read(dir.file()).unwrap(),
                bytes,
                "{fault}: unchanged"
            );
            assert_eq!(dir.names(), vec![FILE_NAME.to_string()], "{fault}");
        }
        let dir = Dir::new();
        std::fs::create_dir(dir.file()).unwrap();
        let read = InstanceKeyFile::in_dir(&dir.0).key_file();
        assert_eq!(
            state(&read),
            "unreadable",
            "a directory where the file belongs"
        );
    }

    #[test]
    fn an_existing_file_with_wider_bits_is_used_and_left_as_it_is() {
        let generated = SecretsKey::generate().unwrap();
        let bytes = format!("{}\n", generated.text.expose_secret()).into_bytes();
        let dir = Dir::new();
        dir.plant(&bytes, 0o644);
        let id = held_id(InstanceKeyFile::in_dir(&dir.0).key_file());
        assert_eq!(id, generated.key.key_id().to_string());
        let meta = std::fs::metadata(dir.file()).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o644, "never chmod-ed");
        assert_eq!(std::fs::read(dir.file()).unwrap(), bytes, "never rewritten");
    }
}
