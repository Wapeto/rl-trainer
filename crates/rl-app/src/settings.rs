//! User settings, stored in the `setting` table (so they travel with backups).
//!
//! Settings are written outside the engine actor on short-lived connections; they are rare and
//! SQLite's busy timeout serialises them with recording writes.

use std::ops::RangeInclusive;
use std::path::PathBuf;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rl_core::tracker::TrackerConfig;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

pub const DEFAULT_PHONE_PORT: u16 = 49_321;
pub const PORT_RANGE: RangeInclusive<u16> = 1024..=65535;
pub const TIMEOUT_RANGE_MIN: RangeInclusive<u32> = 5..=240;
pub const ME_HINT_MAX_CHARS: usize = 64;
pub const HARD_HIT_RANGE: RangeInclusive<f64> = 20.0..=200.0;

const PHONE_ENABLED: &str = "phone_enabled";
const PHONE_PORT: &str = "phone_port";
const PHONE_TOKEN: &str = "phone_token";
const SESSION_TIMEOUT_MIN: &str = "session_timeout_min";
const HARD_HIT_KMH: &str = "hard_hit_kmh";
const INSTALL_DIR: &str = "install_dir";
const REPLAY_DIR: &str = "replay_dir";
const FIREWALL_PORT: &str = "firewall_port";
const ME_HINT: &str = "me_hint";
const SETUP_DISMISSED: &str = "setup_dismissed";
const DEV_DB_IMPORTED_AT: &str = "dev_db_imported_at";
const BACKFILL_DONE: &str = "backfill_done";
const USAGE_PING: &str = "usage_ping";
const INSTALL_ID: &str = "install_id";
const LAST_PING_AT: &str = "last_ping_at";

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("{0}")]
    Invalid(String),
    #[error("settings storage failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("could not generate a random token: {0}")]
    Random(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Settings {
    pub phone_enabled: bool,
    pub phone_port: u16,
    pub session_timeout_min: u32,
    pub hard_hit_kmh: f64,
    #[ts(type = "string | null")]
    pub install_dir: Option<PathBuf>,
    /// The replay folder chosen in Settings; `None` uses the game's folders.
    #[ts(type = "string | null")]
    pub replay_dir: Option<PathBuf>,
    pub firewall_port: Option<u16>,
    pub dev_db_imported_at: Option<i64>,
    /// What the user typed as "me": an in-game name or an account id.
    pub me_hint: Option<String>,
    /// The "Get started" card on Sessions was dismissed.
    pub setup_dismissed: bool,
    /// Sends a random install id and the app version to the relay once a day.
    pub usage_ping: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            phone_enabled: false,
            phone_port: DEFAULT_PHONE_PORT,
            session_timeout_min: 30,
            hard_hit_kmh: 80.0,
            install_dir: None,
            replay_dir: None,
            firewall_port: None,
            dev_db_imported_at: None,
            me_hint: None,
            setup_dismissed: false,
            usage_ping: true,
        }
    }
}

impl Settings {
    /// "Me" comes from the active profile in the database, not from settings.
    pub fn tracker_config(&self) -> TrackerConfig {
        TrackerConfig {
            me_primary_id: None,
            me_hint: self.me_hint.clone(),
            session_timeout_ms: i64::from(self.session_timeout_min) * 60_000,
            hard_hit_kmh: self.hard_hit_kmh,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsPatch {
    pub phone_port: Option<u16>,
    pub session_timeout_min: Option<u32>,
    pub hard_hit_kmh: Option<f64>,
    /// `Some(None)` clears the manual install folder.
    #[serde(default, with = "double_option")]
    pub install_dir: Option<Option<PathBuf>>,
    /// `Some(None)` clears the manual replay folder.
    #[serde(default, with = "double_option")]
    pub replay_dir: Option<Option<PathBuf>>,
    /// Trimmed, at most 64 characters; empty clears it.
    pub me_hint: Option<String>,
    pub setup_dismissed: Option<bool>,
    pub usage_ping: Option<bool>,
}

mod double_option {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Option::<T>::deserialize(deserializer).map(Some)
    }
}

fn get(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM setting WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .optional()
}

fn put(conn: &Connection, key: &str, value: Option<String>) -> rusqlite::Result<()> {
    match value {
        Some(value) => conn.execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?,
        None => conn.execute("DELETE FROM setting WHERE key = ?1", params![key])?,
    };
    Ok(())
}

fn parsed<T: std::str::FromStr>(conn: &Connection, key: &str, default: T) -> rusqlite::Result<T> {
    Ok(match get(conn, key)? {
        Some(text) => text.parse().unwrap_or_else(|_| {
            tracing::warn!(key, value = %text, "ignoring an unreadable setting");
            default
        }),
        None => default,
    })
}

pub fn load(conn: &Connection) -> Result<Settings, SettingsError> {
    let d = Settings::default();
    Ok(Settings {
        phone_enabled: parsed(conn, PHONE_ENABLED, d.phone_enabled)?,
        phone_port: parsed(conn, PHONE_PORT, d.phone_port)?,
        session_timeout_min: parsed(conn, SESSION_TIMEOUT_MIN, d.session_timeout_min)?,
        hard_hit_kmh: parsed(conn, HARD_HIT_KMH, d.hard_hit_kmh)?,
        install_dir: get(conn, INSTALL_DIR)?.map(PathBuf::from),
        replay_dir: get(conn, REPLAY_DIR)?.map(PathBuf::from),
        firewall_port: get(conn, FIREWALL_PORT)?.and_then(|v| v.parse().ok()),
        dev_db_imported_at: get(conn, DEV_DB_IMPORTED_AT)?.and_then(|v| v.parse().ok()),
        me_hint: get(conn, ME_HINT)?,
        setup_dismissed: parsed(conn, SETUP_DISMISSED, d.setup_dismissed)?,
        usage_ping: parsed(conn, USAGE_PING, d.usage_ping)?,
    })
}

pub fn apply_patch(conn: &Connection, patch: &SettingsPatch) -> Result<Settings, SettingsError> {
    if let Some(port) = patch.phone_port {
        if !PORT_RANGE.contains(&port) {
            return Err(SettingsError::Invalid(
                "port must be between 1024 and 65535".into(),
            ));
        }
    }
    if let Some(min) = patch.session_timeout_min {
        if !TIMEOUT_RANGE_MIN.contains(&min) {
            return Err(SettingsError::Invalid(
                "session timeout must be between 5 and 240 minutes".into(),
            ));
        }
    }
    if let Some(kmh) = patch.hard_hit_kmh {
        if !HARD_HIT_RANGE.contains(&kmh) {
            return Err(SettingsError::Invalid(
                "hard-hit threshold must be between 20 and 200 km/h".into(),
            ));
        }
    }
    let me_hint = match &patch.me_hint {
        Some(text) => {
            let text = text.trim();
            if text.chars().count() > ME_HINT_MAX_CHARS {
                return Err(SettingsError::Invalid(
                    "name or account id must be 64 characters or fewer".into(),
                ));
            }
            Some((!text.is_empty()).then(|| text.to_owned()))
        }
        None => None,
    };
    let tx = conn.unchecked_transaction()?;
    if let Some(hint) = me_hint {
        put(&tx, ME_HINT, hint)?;
    }
    if let Some(done) = patch.setup_dismissed {
        put(&tx, SETUP_DISMISSED, Some(done.to_string()))?;
    }
    if let Some(on) = patch.usage_ping {
        put(&tx, USAGE_PING, Some(on.to_string()))?;
    }
    if let Some(port) = patch.phone_port {
        put(&tx, PHONE_PORT, Some(port.to_string()))?;
    }
    if let Some(min) = patch.session_timeout_min {
        put(&tx, SESSION_TIMEOUT_MIN, Some(min.to_string()))?;
    }
    if let Some(kmh) = patch.hard_hit_kmh {
        put(&tx, HARD_HIT_KMH, Some(kmh.to_string()))?;
    }
    if let Some(dir) = &patch.install_dir {
        put(
            &tx,
            INSTALL_DIR,
            dir.as_ref().map(|d| d.to_string_lossy().into_owned()),
        )?;
    }
    if let Some(dir) = &patch.replay_dir {
        put(
            &tx,
            REPLAY_DIR,
            dir.as_ref().map(|d| d.to_string_lossy().into_owned()),
        )?;
    }
    tx.commit()?;
    load(conn)
}

pub fn set_phone_enabled(conn: &Connection, enabled: bool) -> Result<(), SettingsError> {
    Ok(put(conn, PHONE_ENABLED, Some(enabled.to_string()))?)
}

pub fn set_firewall_port(conn: &Connection, port: u16) -> Result<(), SettingsError> {
    Ok(put(conn, FIREWALL_PORT, Some(port.to_string()))?)
}

/// Whether the first-start import of past games has run.
pub fn backfill_done(conn: &Connection) -> Result<bool, SettingsError> {
    Ok(parsed(conn, BACKFILL_DONE, false)?)
}

pub fn set_backfill_done(conn: &Connection) -> Result<(), SettingsError> {
    Ok(put(conn, BACKFILL_DONE, Some("true".to_owned()))?)
}

pub fn set_dev_db_imported_at(conn: &Connection, at_ms: i64) -> Result<(), SettingsError> {
    Ok(put(conn, DEV_DB_IMPORTED_AT, Some(at_ms.to_string()))?)
}

/// This install's random id (32 hex characters), made on first use.
pub fn install_id(conn: &Connection) -> Result<String, SettingsError> {
    if let Some(id) = get(conn, INSTALL_ID)? {
        return Ok(id);
    }
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| SettingsError::Random(e.to_string()))?;
    let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    put(conn, INSTALL_ID, Some(id.clone()))?;
    Ok(id)
}

pub fn last_ping_at(conn: &Connection) -> Result<Option<i64>, SettingsError> {
    Ok(get(conn, LAST_PING_AT)?.and_then(|v| v.parse().ok()))
}

pub fn set_last_ping_at(conn: &Connection, at_ms: i64) -> Result<(), SettingsError> {
    Ok(put(conn, LAST_PING_AT, Some(at_ms.to_string()))?)
}

/// 128 random bits, base64url without padding (22 characters).
pub fn generate_token() -> Result<String, SettingsError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| SettingsError::Random(e.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub fn token(conn: &Connection) -> Result<String, SettingsError> {
    match get(conn, PHONE_TOKEN)? {
        Some(token) => Ok(token),
        None => regenerate_token(conn),
    }
}

pub fn regenerate_token(conn: &Connection) -> Result<String, SettingsError> {
    let token = generate_token()?;
    put(conn, PHONE_TOKEN, Some(token.clone()))?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rl_core::store::open_in_memory;
    use std::path::Path;

    #[test]
    fn defaults_apply_to_a_fresh_database() {
        let conn = open_in_memory().unwrap();
        let s = load(&conn).unwrap();
        assert_eq!(s, Settings::default());
        assert_eq!(s.phone_port, DEFAULT_PHONE_PORT);
        assert_eq!(s.tracker_config().session_timeout_ms, 30 * 60 * 1000);
    }

    #[test]
    fn patches_are_validated_and_persisted() {
        let conn = open_in_memory().unwrap();
        let bad = |patch: SettingsPatch| {
            matches!(apply_patch(&conn, &patch), Err(SettingsError::Invalid(_)))
        };
        assert!(bad(SettingsPatch {
            phone_port: Some(80),
            ..Default::default()
        }));
        assert!(bad(SettingsPatch {
            session_timeout_min: Some(300),
            ..Default::default()
        }));
        assert!(bad(SettingsPatch {
            hard_hit_kmh: Some(10.0),
            ..Default::default()
        }));
        assert!(bad(SettingsPatch {
            hard_hit_kmh: Some(f64::NAN),
            ..Default::default()
        }));
        let s = apply_patch(
            &conn,
            &SettingsPatch {
                phone_port: Some(50_000),
                session_timeout_min: Some(45),
                hard_hit_kmh: Some(90.0),
                install_dir: Some(Some(PathBuf::from("E:/Games/rocketleague"))),
                replay_dir: None,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(load(&conn).unwrap(), s);
        assert_eq!(
            s.install_dir.as_deref(),
            Some(Path::new("E:/Games/rocketleague"))
        );
        let cleared = apply_patch(
            &conn,
            &SettingsPatch {
                install_dir: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(cleared.install_dir, None);
        set_phone_enabled(&conn, true).unwrap();
        set_firewall_port(&conn, 50_000).unwrap();
        set_dev_db_imported_at(&conn, 7).unwrap();
        let s = load(&conn).unwrap();
        assert_eq!(
            (s.phone_enabled, s.firewall_port, s.dev_db_imported_at),
            (true, Some(50_000), Some(7))
        );
    }

    #[test]
    fn the_account_hint_is_trimmed_limited_and_clearable() {
        let conn = open_in_memory().unwrap();
        let hint = |text: &str| {
            apply_patch(
                &conn,
                &SettingsPatch {
                    me_hint: Some(text.into()),
                    ..Default::default()
                },
            )
        };
        assert_eq!(hint("  Name  ").unwrap().me_hint.as_deref(), Some("Name"));
        assert_eq!(
            Settings::default().tracker_config().me_hint,
            None,
            "no hint by default"
        );
        assert!(matches!(
            hint(&"x".repeat(65)),
            Err(SettingsError::Invalid(_))
        ));
        assert_eq!(load(&conn).unwrap().me_hint.as_deref(), Some("Name"));
        assert_eq!(hint("   ").unwrap().me_hint, None);
        let s = apply_patch(
            &conn,
            &SettingsPatch {
                setup_dismissed: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(s.setup_dismissed);
    }

    #[test]
    fn unknown_patch_fields_are_rejected() {
        let err = serde_json::from_str::<SettingsPatch>(r#"{"phone_token":"x"}"#);
        assert!(err.is_err());
    }

    #[test]
    fn token_is_created_once_and_can_be_regenerated() {
        let conn = open_in_memory().unwrap();
        let first = token(&conn).unwrap();
        assert_eq!(
            first.len(),
            22,
            "16 random bytes, base64url without padding"
        );
        assert!(first
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(token(&conn).unwrap(), first);
        let second = regenerate_token(&conn).unwrap();
        assert_ne!(second, first);
        assert_eq!(token(&conn).unwrap(), second);
    }

    #[test]
    fn the_replay_folder_can_be_set_and_cleared() {
        let conn = rl_core::store::open_in_memory().unwrap();
        let set = SettingsPatch {
            replay_dir: Some(Some(PathBuf::from("D:/Replays"))),
            ..SettingsPatch::default()
        };
        assert_eq!(
            apply_patch(&conn, &set).unwrap().replay_dir.as_deref(),
            Some(std::path::Path::new("D:/Replays"))
        );
        let clear = SettingsPatch {
            replay_dir: Some(None),
            ..SettingsPatch::default()
        };
        assert_eq!(apply_patch(&conn, &clear).unwrap().replay_dir, None);
    }
}
