//! `app.db`: global records, starting with the profile list (ADR 0004).
//!
//! Migrations are numbered SQL applied in order, tracked by SQLite's
//! `user_version`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

use crate::ids::ProfileId;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("a profile name cannot be empty")]
    EmptyName,
    #[error("stored profile id {0:?} is not a valid id")]
    CorruptId(String),
    #[error("database schema version {found} is newer than this build supports ({supported})")]
    TooNew { found: i64, supported: i64 },
    #[error("no such profile")]
    NoSuchProfile,
    #[error("the only profile cannot be deleted")]
    LastProfile,
}

pub type Result<T, E = StoreError> = std::result::Result<T, E>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    /// Accent colour as 0xRRGGBB.
    pub accent: u32,
    /// A manually chosen Blobatar; `None` keeps the profile's automatic face.
    pub avatar: Option<AvatarSlot>,
}

/// Stable index into Aeri's bundled Blobatar set. Never reorder those assets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvatarSlot(u8);

impl AvatarSlot {
    pub const COUNT: usize = 64;

    pub fn automatic(profile: ProfileId) -> Self {
        Self((profile.as_u128() % Self::COUNT as u128) as u8)
    }

    pub fn from_index(index: usize) -> Option<Self> {
        (index < Self::COUNT).then_some(Self(index as u8))
    }

    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

impl Profile {
    pub fn avatar_slot(&self) -> AvatarSlot {
        self.avatar
            .unwrap_or_else(|| AvatarSlot::automatic(self.id))
    }
}

/// The name given to the profile created on first launch (stage 1, flow 1).
pub const FIRST_PROFILE_NAME: &str = "Personal";

/// Profile accent colours, in the order new profiles are offered them. They
/// read on Aeri's dark chrome and stay apart for common colour blindness
/// pairs by also differing in lightness.
pub const ACCENTS: [u32; 8] = [
    0x5b7cfa, // blue
    0x3fb68b, // green
    0xf0883e, // orange
    0xe5649a, // pink
    0x9b6dff, // violet
    0x2fb3c9, // teal
    0xe0b43a, // amber
    0xe5534b, // red
];

/// The directories a profile owns under the data directory (ADR 0004).
pub fn profile_dirs(data_dir: &Path, profile: ProfileId) -> [PathBuf; 2] {
    let id = profile.to_string();
    [
        data_dir.join("Profiles").join(&id),
        data_dir.join("Chromium").join(&id),
    ]
}

const MIGRATIONS: &[&str] = &[
    // 1: profiles
    "CREATE TABLE profiles (
        id         TEXT PRIMARY KEY NOT NULL,
        name       TEXT NOT NULL CHECK (length(trim(name)) > 0),
        accent     INTEGER NOT NULL,
        created_at INTEGER NOT NULL DEFAULT (unixepoch()),
        position   INTEGER NOT NULL
    );",
    // 2: the one-time uBlock Origin Lite offer per profile (ADR 0008)
    "ALTER TABLE profiles ADD COLUMN blocker_offered INTEGER NOT NULL DEFAULT 0;",
    // 3: the profile to open at launch; deleted profiles awaiting removal
    "ALTER TABLE profiles ADD COLUMN last_used_at INTEGER NOT NULL DEFAULT 0;
     CREATE TABLE deleted_profiles (
        id         TEXT PRIMARY KEY NOT NULL,
        deleted_at INTEGER NOT NULL DEFAULT (unixepoch())
     );",
    // 4: app-wide settings, one value per key.
    "CREATE TABLE settings (key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL);",
    // 5: optional index into the stable 64-face Blobatar set.
    "ALTER TABLE profiles ADD COLUMN avatar INTEGER
     CHECK (avatar IS NULL OR (typeof(avatar) = 'integer' AND avatar BETWEEN 0 AND 63));",
];

const REST_AFTER: &str = "rest_after_minutes";

/// Applies the migrations a database lacks, each in its own transaction,
/// tracked by SQLite's `user_version`. A database from a newer build is
/// refused rather than modified.
pub(crate) fn migrate(conn: &Connection, migrations: &[&str]) -> Result<()> {
    let supported = migrations.len() as i64;
    let found: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if found > supported {
        return Err(StoreError::TooNew { found, supported });
    }
    for (index, sql) in migrations.iter().enumerate().skip(found as usize) {
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

pub struct AppDb {
    conn: Connection,
}

impl AppDb {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::with_connection(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::with_connection(Connection::open_in_memory()?)
    }

    fn with_connection(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&conn, MIGRATIONS)?;
        Ok(Self { conn })
    }

    /// All profiles in the Person's order.
    pub fn profiles(&self) -> Result<Vec<Profile>> {
        let mut statement = self
            .conn
            .prepare("SELECT id, name, accent, avatar FROM profiles ORDER BY position")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, Option<u8>>(3)?.map(AvatarSlot),
            ))
        })?;
        rows.map(|row| {
            let (id, name, accent, avatar) = row?;
            let id = id.parse().map_err(|_| StoreError::CorruptId(id))?;
            Ok(Profile {
                id,
                name,
                accent,
                avatar,
            })
        })
        .collect()
    }

    /// Creates a profile with a fresh id. Names need not be unique, but cannot
    /// be blank (stage 1, Identity rules).
    pub fn create_profile(&self, name: &str, accent: u32) -> Result<Profile> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::EmptyName);
        }
        let profile = Profile {
            id: ProfileId::generate(),
            name: name.to_owned(),
            accent,
            avatar: None,
        };
        self.conn.execute(
            "INSERT INTO profiles (id, name, accent, position)
             VALUES (?1, ?2, ?3, (SELECT coalesce(max(position), -1) + 1 FROM profiles))",
            params![profile.id.to_string(), profile.name, profile.accent],
        )?;
        Ok(profile)
    }

    /// The profile to open at launch: the last used one, else the first,
    /// created on first launch if none exists.
    pub fn launch_profile(&self) -> Result<Profile> {
        let last: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM profiles ORDER BY last_used_at DESC, position LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        match last {
            Some(id) => self.profile(id.parse().map_err(|_| StoreError::CorruptId(id))?),
            None => self.create_profile(FIRST_PROFILE_NAME, ACCENTS[0]),
        }
    }

    pub fn profile(&self, id: ProfileId) -> Result<Profile> {
        self.profiles()?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or(StoreError::NoSuchProfile)
    }

    /// Renames a profile; returns the stored (trimmed) name.
    pub fn rename_profile(&self, id: ProfileId, name: &str) -> Result<String> {
        let name = name.trim();
        if name.is_empty() {
            return Err(StoreError::EmptyName);
        }
        self.update_one(
            "UPDATE profiles SET name = ?2 WHERE id = ?1",
            params![id.to_string(), name],
        )?;
        Ok(name.to_owned())
    }

    pub fn set_accent(&self, id: ProfileId, accent: u32) -> Result<()> {
        self.update_one(
            "UPDATE profiles SET accent = ?2 WHERE id = ?1",
            params![id.to_string(), accent],
        )
    }

    pub fn set_avatar(&self, id: ProfileId, avatar: Option<AvatarSlot>) -> Result<()> {
        self.update_one(
            "UPDATE profiles SET avatar = ?2 WHERE id = ?1",
            params![id.to_string(), avatar.map(|slot| slot.0)],
        )
    }

    /// Records that the profile was just used, so it opens at next launch.
    pub fn mark_used(&self, id: ProfileId) -> Result<()> {
        self.update_one(
            "UPDATE profiles SET last_used_at =
                (SELECT coalesce(max(last_used_at), 0) + 1 FROM profiles) WHERE id = ?1",
            params![id.to_string()],
        )
    }

    /// The accent for a new profile: the palette colour fewest profiles use,
    /// earliest first (stage 1: suggest a colour that tells profiles apart).
    pub fn suggest_accent(&self) -> Result<u32> {
        let profiles = self.profiles()?;
        let uses = |accent: u32| profiles.iter().filter(|p| p.accent == accent).count();
        Ok(ACCENTS
            .into_iter()
            .min_by_key(|&accent| uses(accent))
            .unwrap_or(ACCENTS[0]))
    }

    /// Removes a profile from the list and queues its data for removal by
    /// [`AppDb::remove_deleted_data`]. The only profile cannot be deleted.
    pub fn delete_profile(&self, id: ProfileId) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let count: i64 = tx.query_row("SELECT count(*) FROM profiles", [], |row| row.get(0))?;
        let deleted = tx.execute(
            "DELETE FROM profiles WHERE id = ?1",
            params![id.to_string()],
        )?;
        if deleted == 0 {
            return Err(StoreError::NoSuchProfile);
        }
        if count <= 1 {
            return Err(StoreError::LastProfile);
        }
        tx.execute(
            "INSERT OR IGNORE INTO deleted_profiles (id) VALUES (?1)",
            params![id.to_string()],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Removes the directories of deleted profiles and forgets those that
    /// are fully gone. Call only while CEF is not running, so Chromium never
    /// has a deleted profile open. Returns the profiles still pending.
    pub fn remove_deleted_data(&self, data_dir: &Path) -> Result<Vec<(ProfileId, String)>> {
        let ids: Vec<String> = self
            .conn
            .prepare("SELECT id FROM deleted_profiles")?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut pending = Vec::new();
        for raw in ids {
            let id: ProfileId = raw
                .parse()
                .map_err(|_| StoreError::CorruptId(raw.clone()))?;
            let failure = profile_dirs(data_dir, id).iter().find_map(|dir| {
                match std::fs::remove_dir_all(dir) {
                    Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                        Some(format!("{}: {error}", dir.display()))
                    }
                    _ => None,
                }
            });
            match failure {
                Some(error) => pending.push((id, error)),
                None => {
                    self.conn
                        .execute("DELETE FROM deleted_profiles WHERE id = ?1", params![raw])?;
                }
            }
        }
        Ok(pending)
    }

    /// Claims the profile's one-time offer of an ad blocker (ADR 0008):
    /// true the first time it is asked for this profile, false after.
    pub fn take_blocker_offer(&self, profile: ProfileId) -> Result<bool> {
        let claimed = self.conn.execute(
            "UPDATE profiles SET blocker_offered = 1 WHERE id = ?1 AND blocker_offered = 0",
            params![profile.to_string()],
        )?;
        Ok(claimed == 1)
    }

    /// How long a tab may go unseen before it rests; `None`: never. The
    /// default until the Person chooses is
    /// [`DEFAULT_REST_AFTER`](crate::resources::DEFAULT_REST_AFTER).
    pub fn rest_after(&self) -> Result<Option<Duration>> {
        let minutes = self
            .setting(REST_AFTER)?
            .map(|value| value.parse::<i64>().unwrap_or(0));
        Ok(match minutes {
            None => crate::resources::DEFAULT_REST_AFTER,
            Some(minutes) if minutes <= 0 => None,
            Some(minutes) => Some(Duration::from_secs(minutes as u64 * 60)),
        })
    }

    pub fn set_rest_after(&self, after: Option<Duration>) -> Result<()> {
        let minutes = after.map_or(0, |after| after.as_secs() / 60);
        self.set_setting(REST_AFTER, &minutes.to_string())
    }

    /// An app-wide setting (`settings` table), if set.
    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Runs an update that must touch exactly one profile.
    fn update_one(&self, sql: &str, params: impl rusqlite::Params) -> Result<()> {
        match self.conn.execute(sql, params)? {
            0 => Err(StoreError::NoSuchProfile),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_launch_creates_one_personal_profile_and_reuses_it() {
        let db = AppDb::open_in_memory().unwrap();
        let first = db.launch_profile().unwrap();
        assert_eq!(first.name, FIRST_PROFILE_NAME);
        assert_eq!(db.launch_profile().unwrap(), first);
        assert_eq!(db.profiles().unwrap().len(), 1);
    }

    #[test]
    fn launch_opens_the_last_used_profile() {
        let db = AppDb::open_in_memory().unwrap();
        let personal = db.launch_profile().unwrap();
        let work = db.create_profile("Work", 1).unwrap();
        assert_eq!(db.launch_profile().unwrap(), personal);
        db.mark_used(work.id).unwrap();
        assert_eq!(db.launch_profile().unwrap(), work);
        db.mark_used(personal.id).unwrap();
        assert_eq!(db.launch_profile().unwrap(), personal);
    }

    #[test]
    fn renaming_and_recolouring_keep_the_id() {
        let db = AppDb::open_in_memory().unwrap();
        let work = db.create_profile("Work", ACCENTS[0]).unwrap();
        assert_eq!(db.rename_profile(work.id, "  Client  ").unwrap(), "Client");
        assert!(matches!(
            db.rename_profile(work.id, " "),
            Err(StoreError::EmptyName)
        ));
        db.set_accent(work.id, ACCENTS[3]).unwrap();
        let stored = db.profile(work.id).unwrap();
        assert_eq!(
            (stored.name.as_str(), stored.accent),
            ("Client", ACCENTS[3])
        );
        assert!(matches!(
            db.rename_profile(ProfileId::generate(), "Ghost"),
            Err(StoreError::NoSuchProfile)
        ));
    }

    #[test]
    fn suggested_accents_avoid_colours_in_use() {
        let db = AppDb::open_in_memory().unwrap();
        assert_eq!(db.suggest_accent().unwrap(), ACCENTS[0]);
        db.create_profile("A", ACCENTS[0]).unwrap();
        db.create_profile("B", ACCENTS[2]).unwrap();
        assert_eq!(db.suggest_accent().unwrap(), ACCENTS[1]);
    }

    #[test]
    fn the_only_profile_cannot_be_deleted() {
        let db = AppDb::open_in_memory().unwrap();
        let only = db.launch_profile().unwrap();
        assert!(matches!(
            db.delete_profile(only.id),
            Err(StoreError::LastProfile)
        ));
        assert_eq!(db.profiles().unwrap(), vec![only]);
    }

    #[test]
    fn deleting_removes_the_profile_and_later_its_directories() {
        let data = std::env::temp_dir().join(format!("aeri-delete-test-{}", ProfileId::generate()));
        let db = AppDb::open_in_memory().unwrap();
        let keep = db.launch_profile().unwrap();
        let gone = db.create_profile("Client", 1).unwrap();
        for dir in profile_dirs(&data, keep.id)
            .iter()
            .chain(&profile_dirs(&data, gone.id))
        {
            std::fs::create_dir_all(dir.join("Default")).unwrap();
        }

        db.delete_profile(gone.id).unwrap();
        assert_eq!(db.profiles().unwrap(), vec![keep.clone()]);
        assert!(matches!(
            db.delete_profile(gone.id),
            Err(StoreError::NoSuchProfile)
        ));
        // Nothing is removed until CEF is down.
        assert!(profile_dirs(&data, gone.id).iter().all(|dir| dir.exists()));

        assert!(db.remove_deleted_data(&data).unwrap().is_empty());
        assert!(profile_dirs(&data, gone.id).iter().all(|dir| !dir.exists()));
        assert!(profile_dirs(&data, keep.id).iter().all(|dir| dir.exists()));
        // Forgotten once removed: a second sweep has nothing to do.
        assert!(db.remove_deleted_data(&data).unwrap().is_empty());
        std::fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn profiles_keep_creation_order_and_may_share_names() {
        let db = AppDb::open_in_memory().unwrap();
        let a = db.create_profile("Work", 1).unwrap();
        let b = db.create_profile("Work", 2).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(db.profiles().unwrap(), vec![a, b]);
    }

    #[test]
    fn rest_after_defaults_to_half_an_hour_and_never_is_kept() {
        let db = AppDb::open_in_memory().unwrap();
        assert_eq!(db.rest_after().unwrap(), Some(Duration::from_secs(1_800)));
        db.set_rest_after(None).unwrap();
        assert_eq!(db.rest_after().unwrap(), None);
        db.set_rest_after(Some(Duration::from_secs(900))).unwrap();
        assert_eq!(db.rest_after().unwrap(), Some(Duration::from_secs(900)));
    }

    #[test]
    fn blank_names_are_rejected_and_names_are_trimmed() {
        let db = AppDb::open_in_memory().unwrap();
        assert!(matches!(
            db.create_profile("   ", 0),
            Err(StoreError::EmptyName)
        ));
        assert_eq!(db.create_profile("  Client  ", 0).unwrap().name, "Client");
    }

    #[test]
    fn reopening_a_file_keeps_profiles_and_skips_applied_migrations() {
        let dir = std::env::temp_dir().join(format!("aeri-store-test-{}", ProfileId::generate()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.db");
        let created = AppDb::open(&path).unwrap().launch_profile().unwrap();
        let reopened = AppDb::open(&path).unwrap();
        assert_eq!(reopened.profiles().unwrap(), vec![created]);
        drop(reopened);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn avatars_are_independent_and_reset_after_migrating_an_existing_database() {
        let dir = std::env::temp_dir().join(format!("aeri-store-test-{}", ProfileId::generate()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.db");
        let old = AppDb {
            conn: Connection::open(&path).unwrap(),
        };
        migrate(&old.conn, &MIGRATIONS[..4]).unwrap();
        let work = old.create_profile("Work", ACCENTS[0]).unwrap();
        let personal = old.create_profile("Personal", ACCENTS[1]).unwrap();
        drop(old);

        let db = AppDb::open(&path).unwrap();
        let default = work.avatar_slot();
        assert_eq!(db.profile(work.id).unwrap().avatar_slot(), default);
        let picked = AvatarSlot::from_index((default.index() + 1) % AvatarSlot::COUNT).unwrap();
        db.set_avatar(work.id, Some(picked)).unwrap();
        db.rename_profile(work.id, "Projects").unwrap();
        db.set_accent(work.id, ACCENTS[3]).unwrap();
        drop(db);

        let db = AppDb::open(&path).unwrap();
        assert_eq!(db.profile(work.id).unwrap().avatar_slot(), picked);
        assert_eq!(db.profile(personal.id).unwrap(), personal);
        db.set_avatar(work.id, None).unwrap();
        assert_eq!(db.profile(work.id).unwrap().avatar_slot(), default);
        drop(db);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_newer_schema_is_refused_instead_of_corrupted() {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(
            AppDb::with_connection(conn),
            Err(StoreError::TooNew { found: 99, .. })
        ));
    }

    #[test]
    fn the_blocker_is_offered_once_per_profile() {
        let db = AppDb::open_in_memory().unwrap();
        let personal = db.launch_profile().unwrap();
        let work = db.create_profile("Work", 1).unwrap();
        assert!(db.take_blocker_offer(personal.id).unwrap());
        assert!(!db.take_blocker_offer(personal.id).unwrap());
        assert!(db.take_blocker_offer(work.id).unwrap());
    }
}
