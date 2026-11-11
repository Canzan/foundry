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
//! exists read back. A temp file gone before its link (`ENOENT`) reads a
//! sibling's file when one appeared, else is made once more. Only temp files
//! older than [`STALE_TEMP`] are leftovers: a sibling server on the same
//! volume may be mid-way through its own. A filesystem without hard links
//! (`EPERM`, `ENOTSUP`) cannot hold the file (I-27). No file is made over a database that already
//! holds what an earlier key sealed (r3). An existing `instance.key` is never
//! overwritten, chmod-ed or removed. No key, and no file content, is ever
//! logged.

use super::keys::{InstanceKeySource, KeyFileRead};
use foundry_auth::secret_box::{SecretsKey, INSTANCE_KEY_SETTING};
use secrecy::zeroize::Zeroizing;
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
/// A temp file lives from its open to just after its link: one write and one
/// `fsync`. One untouched this long was left by a start that crashed; a
/// younger one may be a sibling server's in flight, and leaving a leftover a
/// while longer costs nothing (it is never read).
const STALE_TEMP: std::time::Duration = std::time::Duration::from_secs(10 * 60);
/// Tries at making the file: the first, and one more when its temp file was
/// gone before its link.
const MAKE_TRIES: usize = 2;

/// The key file's path in `data_dir`, as the page names it.
pub fn key_path(data_dir: &Path) -> PathBuf {
    data_dir.join(FILE_NAME)
}

/// The instance key file in one data directory: the [`InstanceKeySource`]
/// the shared boot is given.
pub struct InstanceKeyFile {
    dir: PathBuf,
    /// `link(2)`: the one call a filesystem without hard links refuses.
    link: fn(&Path, &Path) -> std::io::Result<()>,
}

impl InstanceKeyFile {
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            link: |from, to| std::fs::hard_link(from, to),
        }
    }

    /// The key file of `settings`' data directory.
    pub fn of(settings: &super::config::IntegrationSettings) -> Self {
        Self::in_dir(settings.data_dir.clone())
    }

    fn path(&self) -> PathBuf {
        key_path(&self.dir)
    }

    /// The probe (ADR-IIS-002 r2, r3, r6): leftover temp files removed,
    /// then the existing file read, or, when `may_make`, a new one made,
    /// linked, read back and checked before its key is used.
    fn probe(&self, may_make: bool) -> KeyFileRead {
        self.remove_leftovers();
        match std::fs::symlink_metadata(self.path()) {
            Ok(_) => self.read_existing(),
            // No file there; a data directory that is no directory cannot
            // take one, which the attempt reports (I-27).
            Err(err) if matches!(err.kind(), ErrorKind::NotFound | ErrorKind::NotADirectory) => {
                if may_make {
                    self.create()
                } else {
                    tracing::warn!(
                        event = "integrations.key_file.withheld",
                        "no instance key file: none is made while the database holds what an earlier instance key sealed"
                    );
                    KeyFileRead::Withheld
                }
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

    /// Temp files a crashed start left behind: older than [`STALE_TEMP`]
    /// (one dated ahead of this clock is not). `instance.key` itself is never
    /// touched. A directory that cannot be listed has none to remove.
    fn remove_leftovers(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let now = std::time::SystemTime::now();
        let stale = |entry: &std::fs::DirEntry| {
            entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|written| now.duration_since(written).ok())
                .is_some_and(|age| age >= STALE_TEMP)
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(TEMP_PREFIX) && stale(&entry) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// A file that was already there: used when it parses, even with wider
    /// bits or another owner than its data directory's (warned, never
    /// chmod-ed or chown-ed); never rewritten when it does not.
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
                if !self.owned_as_its_directory() {
                    tracing::warn!(
                        event = "integrations.key_file.other_owner",
                        "the instance key file belongs to another user than its data directory; it is used and left as it is"
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

    /// Whether the file's owner is the data directory's: the running user's
    /// stand-in, with no libc to ask for the uid.
    fn owned_as_its_directory(&self) -> bool {
        let uid = |path: &Path| std::fs::symlink_metadata(path).map(|meta| meta.uid()).ok();
        uid(&self.path()) == uid(&self.dir)
    }

    /// Make the file once. A directory that cannot take it is `Unwritable`
    /// (I-27); losing the race to another server reads the winner's file. A
    /// temp file gone before its link reads a sibling's file when one is
    /// there, else is made once more.
    fn create(&self) -> KeyFileRead {
        for _ in 0..MAKE_TRIES {
            match self.attempt() {
                Attempt::Read(read) => return read,
                Attempt::TempGone if std::fs::symlink_metadata(self.path()).is_ok() => {
                    return self.read_existing()
                }
                Attempt::TempGone => {}
            }
        }
        unwritable(ErrorKind::NotFound)
    }

    /// One try at making the file.
    fn attempt(&self) -> Attempt {
        let temp = self
            .dir
            .join(format!("{TEMP_PREFIX}{}", uuid::Uuid::new_v4().simple()));
        let made = write_temp(&temp).map_err(Failed::Temp).and_then(|uid| {
            let linked = (self.link)(&temp, &self.path());
            let _ = std::fs::remove_file(&temp);
            match linked {
                Ok(()) => {
                    sync_dir(&self.dir);
                    Ok(Some(uid))
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => Ok(None),
                // EPERM, ENOTSUP and the rest: this filesystem cannot hold it.
                Err(err) => Err(Failed::Link(err)),
            }
        });
        match made {
            Ok(Some(uid)) => Attempt::Read(self.read_created(uid)),
            Ok(None) => Attempt::Read(self.read_existing()),
            Err(Failed::Link(err)) if err.kind() == ErrorKind::NotFound => Attempt::TempGone,
            Err(Failed::Link(err) | Failed::Temp(err)) => {
                let _ = std::fs::remove_file(&temp);
                Attempt::Read(unwritable(err.kind()))
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

/// One try at making the file: read through, or our temp file was gone
/// before its link.
enum Attempt {
    Read(KeyFileRead),
    TempGone,
}

/// Where a try failed: writing the temp file (a missing directory is
/// `ENOENT` there, never retried) or linking it.
enum Failed {
    Temp(std::io::Error),
    Link(std::io::Error),
}

/// I-27, logged by kind only.
fn unwritable(kind: ErrorKind) -> KeyFileRead {
    tracing::warn!(
        event = "integrations.key_file.unwritable",
        error_kind = ?kind,
        "the instance key file cannot be made"
    );
    KeyFileRead::Unwritable
}

impl InstanceKeySource for InstanceKeyFile {
    fn key_file(&self, may_make: bool) -> KeyFileRead {
        self.probe(may_make)
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
    let mut line = key_buffer();
    line.push_str(generated.text.expose_secret());
    line.push('\n');
    file.write_all(line.as_bytes())?;
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

/// A buffer for the key file's text, zeroed on drop and sized past
/// [`MAX_FILE_BYTES`] so neither a read nor a write reallocates it (which
/// would leave a copy of the key behind).
fn key_buffer() -> Zeroizing<String> {
    Zeroizing::new(String::with_capacity(MAX_FILE_BYTES as usize + 1))
}

/// The key in the regular file at `path`, and the file's permission bits;
/// the problem otherwise (never the content).
fn read_key(path: &Path) -> Result<(SecretsKey, u32), &'static str> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| "no metadata")?;
    if !meta.file_type().is_file() {
        return Err("not a regular file");
    }
    let mut text = key_buffer();
    File::open(path)
        .map_err(|_| "cannot open")?
        .take(MAX_FILE_BYTES)
        .read_to_string(&mut text)
        .map_err(|_| "cannot read")?;
    let line = SecretString::from(text.trim());
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
            KeyFileRead::Withheld => panic!("expected a held key, got Withheld"),
        }
    }

    fn state(read: &KeyFileRead) -> &'static str {
        match read {
            KeyFileRead::Held(_) => "held",
            KeyFileRead::Unwritable => "unwritable",
            KeyFileRead::Unreadable => "unreadable",
            KeyFileRead::Withheld => "withheld",
        }
    }

    #[test]
    fn an_absent_file_is_made_once_private_and_read_back_the_same() {
        let dir = Dir::new();
        let first = held_id(InstanceKeyFile::in_dir(&dir.0).key_file(true));
        let meta = std::fs::metadata(dir.file()).expect("the file was made");
        assert_eq!(meta.permissions().mode() & 0o777, PRIVATE_MODE);
        let bytes = std::fs::read(dir.file()).expect("read the file");
        let second = held_id(InstanceKeyFile::in_dir(&dir.0).key_file(true));
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
                .map(|_| s.spawn(|| held_id(InstanceKeyFile::in_dir(&dir.0).key_file(true))))
                .collect();
            racers.into_iter().map(|r| r.join().unwrap()).collect()
        });
        assert!(ids.iter().all(|id| *id == ids[0]), "{ids:?}");
        assert_eq!(dir.names(), vec![FILE_NAME.to_string()]);
    }

    /// A temp file last written `age` ago.
    fn plant_temp(dir: &Dir, name: &str, age: std::time::Duration) -> PathBuf {
        let temp = dir.0.join(format!("{TEMP_PREFIX}{name}"));
        std::fs::write(&temp, b"partial").unwrap();
        let written = std::time::SystemTime::now() - age;
        File::options()
            .write(true)
            .open(&temp)
            .and_then(|f| f.set_modified(written))
            .expect("age the temp file");
        temp
    }

    const AN_HOUR: std::time::Duration = std::time::Duration::from_secs(3600);

    #[test]
    fn a_leftover_temp_file_is_removed_and_the_file_made() {
        let dir = Dir::new();
        plant_temp(&dir, "crashed", AN_HOUR);
        held_id(InstanceKeyFile::in_dir(&dir.0).key_file(true));
        assert_eq!(dir.names(), vec![FILE_NAME.to_string()]);
    }

    /// H1: a sibling server sharing the volume is mid-way through making the
    /// file; its fresh temp file is not this server's to remove.
    #[test]
    fn a_siblings_temp_file_in_flight_is_not_removed() {
        let dir = Dir::new();
        let sibling = plant_temp(&dir, "sibling", std::time::Duration::ZERO);
        held_id(InstanceKeyFile::in_dir(&dir.0).key_file(true));
        assert!(sibling.exists(), "the sibling's temp file is left alone");
        let ahead = plant_temp(&dir, "clock-ahead", std::time::Duration::ZERO);
        File::options()
            .write(true)
            .open(&ahead)
            .and_then(|f| f.set_modified(std::time::SystemTime::now() + AN_HOUR))
            .unwrap();
        held_id(InstanceKeyFile::in_dir(&dir.0).key_file(false));
        assert!(
            ahead.exists(),
            "a temp file from a clock ahead is not stale"
        );
        assert!(sibling.exists());
    }

    /// Losing the race at `link` (`EEXIST`): the winner's file is read.
    #[test]
    fn losing_the_link_race_reads_the_winners_file() {
        let dir = Dir::new();
        let file = InstanceKeyFile {
            link: |_, to| {
                let winner = SecretsKey::generate().unwrap();
                std::fs::write(to, format!("{}\n", winner.text.expose_secret()))?;
                Err(ErrorKind::AlreadyExists.into())
            },
            ..InstanceKeyFile::in_dir(&dir.0)
        };
        let id = held_id(file.key_file(true));
        let (winner, _) = read_key(&dir.file()).expect("the winner's file");
        assert_eq!(id, winner.key_id().to_string());
        assert_eq!(dir.names(), vec![FILE_NAME.to_string()], "our temp removed");
    }

    /// Our temp file gone before its `link` (`ENOENT`): made once more.
    #[test]
    fn a_temp_file_gone_before_its_link_is_made_once_more() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static LINKS: AtomicUsize = AtomicUsize::new(0);
        let dir = Dir::new();
        let file = InstanceKeyFile {
            link: |from, to| {
                if LINKS.fetch_add(1, Ordering::SeqCst) == 0 {
                    std::fs::remove_file(from)?;
                }
                std::fs::hard_link(from, to)
            },
            ..InstanceKeyFile::in_dir(&dir.0)
        };
        let id = held_id(file.key_file(true));
        assert_eq!(LINKS.load(Ordering::SeqCst), 2, "one retry");
        let (made, mode) = read_key(&dir.file()).expect("the file made on retry");
        assert_eq!((id, mode), (made.key_id().to_string(), PRIVATE_MODE));
        assert_eq!(dir.names(), vec![FILE_NAME.to_string()]);
    }

    /// `ENOENT` at `link` while a sibling's file appeared: the sibling's is
    /// read; `ENOENT` every time: one retry, then I-27.
    #[test]
    fn a_link_that_finds_no_temp_file_reads_a_siblings_file_or_gives_up() {
        let dir = Dir::new();
        let file = InstanceKeyFile {
            link: |_, to| {
                let winner = SecretsKey::generate().unwrap();
                std::fs::write(to, format!("{}\n", winner.text.expose_secret()))?;
                Err(ErrorKind::NotFound.into())
            },
            ..InstanceKeyFile::in_dir(&dir.0)
        };
        let id = held_id(file.key_file(true));
        let (winner, _) = read_key(&dir.file()).expect("the sibling's file");
        assert_eq!(id, winner.key_id().to_string());

        use std::sync::atomic::{AtomicUsize, Ordering};
        static LINKS: AtomicUsize = AtomicUsize::new(0);
        let dir = Dir::new();
        let file = InstanceKeyFile {
            link: |_, _| {
                LINKS.fetch_add(1, Ordering::SeqCst);
                Err(ErrorKind::NotFound.into())
            },
            ..InstanceKeyFile::in_dir(&dir.0)
        };
        assert_eq!(state(&file.key_file(true)), "unwritable");
        assert_eq!(LINKS.load(Ordering::SeqCst), 2, "one retry, no more");
        assert!(dir.names().is_empty(), "no file and no temp file");
    }

    #[test]
    fn a_directory_that_cannot_take_the_file_is_unwritable() {
        // A path under a regular file can never be a directory, whoever runs
        // the test; a read-only directory is one only for a non-root user.
        let dir = Dir::new();
        let blocker = dir.0.join("not-a-directory");
        std::fs::write(&blocker, b"a file\n").unwrap();
        let read = InstanceKeyFile::in_dir(&blocker).key_file(true);
        assert_eq!(state(&read), "unwritable");

        let read_only = Dir::new();
        std::fs::set_permissions(&read_only.0, std::fs::Permissions::from_mode(0o500)).unwrap();
        let writable_anyway = File::create(read_only.0.join("probe")).is_ok();
        let read = InstanceKeyFile::in_dir(&read_only.0).key_file(true);
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
            state(&InstanceKeyFile::in_dir(&missing).key_file(true)),
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
            let read = InstanceKeyFile::in_dir(&dir.0).key_file(true);
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
        let read = InstanceKeyFile::in_dir(&dir.0).key_file(true);
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
        let id = held_id(InstanceKeyFile::in_dir(&dir.0).key_file(true));
        assert_eq!(id, generated.key.key_id().to_string());
        let meta = std::fs::metadata(dir.file()).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o644, "never chmod-ed");
        assert_eq!(std::fs::read(dir.file()).unwrap(), bytes, "never rewritten");
    }

    #[test]
    fn a_filesystem_without_hard_links_is_unwritable_and_keeps_nothing() {
        for (fault, error) in [
            ("link EPERM", ErrorKind::PermissionDenied),
            ("link ENOTSUP", ErrorKind::Unsupported),
        ] {
            let dir = Dir::new();
            let file = InstanceKeyFile {
                link: match error {
                    ErrorKind::PermissionDenied => |_, _| Err(ErrorKind::PermissionDenied.into()),
                    _ => |_, _| Err(ErrorKind::Unsupported.into()),
                },
                ..InstanceKeyFile::in_dir(&dir.0)
            };
            assert_eq!(state(&file.key_file(true)), "unwritable", "{fault}");
            assert!(dir.names().is_empty(), "{fault}: no file and no temp file");
        }
    }

    #[test]
    fn no_file_is_made_over_a_database_an_earlier_key_sealed() {
        let dir = Dir::new();
        let read = InstanceKeyFile::in_dir(&dir.0).key_file(false);
        assert_eq!(state(&read), "withheld");
        assert!(dir.names().is_empty(), "no key file minted");

        let leftover = Dir::new();
        plant_temp(&leftover, "crashed", AN_HOUR);
        let read = InstanceKeyFile::in_dir(&leftover.0).key_file(false);
        assert_eq!(state(&read), "withheld");
        assert!(leftover.names().is_empty(), "the leftover is still removed");

        let generated = SecretsKey::generate().unwrap();
        let existing = Dir::new();
        existing.plant(
            format!("{}\n", generated.text.expose_secret()).as_bytes(),
            PRIVATE_MODE,
        );
        let id = held_id(InstanceKeyFile::in_dir(&existing.0).key_file(false));
        assert_eq!(
            id,
            generated.key.key_id().to_string(),
            "an existing file is read"
        );
    }

    /// What `probe` returns, and every line it logged.
    fn logged(probe: impl FnOnce() -> KeyFileRead) -> (KeyFileRead, String) {
        #[derive(Clone, Default)]
        struct Buf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
        impl std::io::Write for Buf {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().expect("buf").extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let buf = Buf::default();
        let sink = buf.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(move || sink.clone())
            .with_ansi(false)
            .finish();
        // A callsite a parallel test reached first, with no subscriber, may
        // have cached "never"; re-judge every callsite with this one in place.
        let read = tracing::subscriber::with_default(subscriber, || {
            tracing::callsite::rebuild_interest_cache();
            probe()
        });
        let out = String::from_utf8(buf.0.lock().expect("buf").clone()).expect("utf8");
        (read, out)
    }

    /// An existing file is warned about only for bits beyond its owner's,
    /// and never for an owner its data directory shares.
    #[test]
    fn only_a_file_readable_beyond_its_owner_is_warned_about() {
        let generated = SecretsKey::generate().unwrap();
        let bytes = format!("{}\n", generated.text.expose_secret()).into_bytes();
        for (mode, warned) in [
            (PRIVATE_MODE, false),
            (0o400, false),
            (0o640, true),
            (0o604, true),
        ] {
            let dir = Dir::new();
            dir.plant(&bytes, mode);
            let (read, out) = logged(|| InstanceKeyFile::in_dir(&dir.0).key_file(true));
            assert_eq!(state(&read), "held", "{mode:o}");
            assert_eq!(
                out.contains("integrations.key_file.wide_mode"),
                warned,
                "{mode:o}: {out}"
            );
            assert!(
                !out.contains("integrations.key_file.other_owner"),
                "{mode:o}: {out}"
            );
        }
    }

    /// ADR-IIS-002 r6: a temp file is a leftover only once it is ten minutes
    /// old; a younger one may still be a sibling's in flight.
    #[test]
    fn only_temp_files_ten_minutes_old_are_leftovers() {
        let dir = Dir::new();
        let young = plant_temp(&dir, "nine-minutes", std::time::Duration::from_secs(9 * 60));
        let old = plant_temp(
            &dir,
            "eleven-minutes",
            std::time::Duration::from_secs(11 * 60),
        );
        held_id(InstanceKeyFile::in_dir(&dir.0).key_file(true));
        assert!(young.exists(), "a nine-minute-old temp file is kept");
        assert!(!old.exists(), "an eleven-minute-old temp file is removed");
    }

    /// A data directory whose key file cannot even be looked up for a
    /// reason other than its absence (here a name too long for any
    /// filesystem) is unreadable, never a place to make one.
    #[test]
    fn a_key_file_that_cannot_be_looked_up_is_unreadable() {
        let dir = Dir::new();
        let unnameable = dir.0.join("d".repeat(300));
        let read = InstanceKeyFile::in_dir(&unnameable).key_file(true);
        assert_eq!(state(&read), "unreadable");
        assert!(dir.names().is_empty(), "nothing made");
    }

    /// The file this server just linked is checked before its key is used:
    /// one that is not 0600 by then fails the check (I-27).
    #[test]
    fn a_made_file_that_fails_its_check_is_unwritable() {
        let dir = Dir::new();
        let file = InstanceKeyFile {
            link: |from, to| {
                std::fs::hard_link(from, to)?;
                std::fs::set_permissions(to, std::fs::Permissions::from_mode(0o644))
            },
            ..InstanceKeyFile::in_dir(&dir.0)
        };
        assert_eq!(state(&file.key_file(true)), "unwritable");
    }

    /// I-27 from `link` (`EPERM`, `ENOTSUP`) is final: one try, no retry
    /// (only a temp file gone, `ENOENT`, is made once more).
    #[test]
    fn a_link_the_filesystem_refuses_is_tried_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static LINKS: AtomicUsize = AtomicUsize::new(0);
        let dir = Dir::new();
        let file = InstanceKeyFile {
            link: |_, _| {
                LINKS.fetch_add(1, Ordering::SeqCst);
                Err(ErrorKind::PermissionDenied.into())
            },
            ..InstanceKeyFile::in_dir(&dir.0)
        };
        assert_eq!(state(&file.key_file(true)), "unwritable");
        assert_eq!(LINKS.load(Ordering::SeqCst), 1, "no retry");
        assert!(dir.names().is_empty(), "no file and no temp file");
    }

    /// The boot's store: one snapshot, whatever is asked.
    struct Stored(foundry_store::integrations::InstanceSnapshot);

    #[async_trait::async_trait]
    impl super::super::store_capabilities::KeyProbe for Stored {
        async fn record_key_check(
            &self,
            _check: &foundry_store::integrations::KeyCheck,
        ) -> Result<(), foundry_store::StoreError> {
            Ok(())
        }
        async fn instance_snapshot(
            &self,
        ) -> Result<foundry_store::integrations::InstanceSnapshot, foundry_store::StoreError>
        {
            Ok(self.0.clone())
        }
        async fn rewrap_secrets_key(
            &self,
            _rewrap: &foundry_store::integrations::SecretsKeyRewrap,
            _at: time::OffsetDateTime,
        ) -> Result<foundry_store::integrations::RewrapOutcome, foundry_store::StoreError> {
            Ok(foundry_store::integrations::RewrapOutcome::AlreadyRewrapped)
        }
    }

    /// Booted with no operator key on `dir` over `snapshot`.
    async fn boot_over(
        dir: &Dir,
        snapshot: foundry_store::integrations::InstanceSnapshot,
    ) -> super::super::Integrations {
        let mut settings =
            super::super::config::IntegrationSettings::off("https://foundry.example.org");
        settings.data_dir = dir.0.clone();
        let key_file = InstanceKeyFile::of(&settings);
        super::super::Integrations::boot(
            settings,
            &Stored(snapshot),
            &key_file,
            time::OffsetDateTime::now_utc(),
        )
        .await
        .expect("boot")
    }

    fn snapshot() -> foundry_store::integrations::InstanceSnapshot {
        foundry_store::integrations::InstanceSnapshot {
            version: 1,
            secrets_key: None,
            slack_app: None,
            slack_app_pending: None,
            key_checks: Vec::new(),
            sealed_credentials: Vec::new(),
        }
    }

    /// ADR-IIS-002 r3 through the boot: a stored secrets key, or a value
    /// sealed directly, and no file: none is made, and the page reads I-26
    /// or I-4, never I-27. An empty database gets its file.
    #[tokio::test]
    async fn a_stored_key_with_no_file_mints_none_and_reads_unopenable() {
        use super::super::keys::KeyMode;
        let earlier = SecretsKey::generate().unwrap();
        let earlier = foundry_auth::secret_box::SecretBox::new(earlier.key, Vec::new());

        let dir = Dir::new();
        let stored_key = foundry_store::integrations::InstanceSnapshot {
            secrets_key: Some(foundry_store::integrations::StoredSecretsKey {
                key_id: "an-earlier-secrets-key".to_string(),
                wrapped_key: vec![7; 48],
                wrapped_by_key_id: earlier.current_key_id().to_string(),
                wrapped_by: foundry_store::integrations::KeyWrapper::File,
                rewrapped_from_path: None,
                created_by: None,
                created_at: time::OffsetDateTime::UNIX_EPOCH,
            }),
            ..snapshot()
        };
        let booted = boot_over(&dir, stored_key).await;
        assert_eq!(booted.key_mode(), KeyMode::Unopenable, "I-26");
        assert!(
            dir.names().is_empty(),
            "no file minted over a stored secrets key"
        );

        let dir = Dir::new();
        let sealed = earlier
            .seal(
                &SecretString::from("xoxb-a-bot-token"),
                &foundry_auth::secret_box::SecretSlot::new(
                    "integration_connections",
                    "1",
                    "credential",
                ),
            )
            .expect("seal");
        let direct = foundry_store::integrations::InstanceSnapshot {
            sealed_credentials: vec![sealed.as_bytes().to_vec()],
            ..snapshot()
        };
        let booted = boot_over(&dir, direct).await;
        assert_eq!(booted.key_mode(), KeyMode::AwaitingSecretsKey);
        assert!(booted.secrets_check().degraded(), "I-4");
        assert!(dir.names().is_empty(), "no file minted over a direct seal");

        let dir = Dir::new();
        let fresh = boot_over(&dir, snapshot()).await;
        assert_eq!(fresh.key_mode(), KeyMode::AwaitingSecretsKey);
        assert!(!fresh.secrets_check().degraded());
        assert_eq!(
            dir.names(),
            vec![FILE_NAME.to_string()],
            "an empty database gets its file"
        );
    }
}
