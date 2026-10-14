//! Forward-only migrations tracked with `PRAGMA user_version`.

use rusqlite::Connection;

const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_init.sql"),
    include_str!("../../migrations/0002_app.sql"),
    include_str!("../../migrations/0003_aerial_shots_pass.sql"),
    include_str!("../../migrations/0004_review_and_misses.sql"),
    include_str!("../../migrations/0005_pack_hidden.sql"),
    include_str!("../../migrations/0006_lobby_games_and_forfeits.sql"),
    include_str!("../../migrations/0007_match_metrics.sql"),
    include_str!("../../migrations/0008_goalie_packs.sql"),
    include_str!("../../migrations/0009_replays.sql"),
    include_str!("../../migrations/0010_deleted_matches.sql"),
    include_str!("../../migrations/0011_replay_players.sql"),
];

pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

pub fn user_version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.pragma_query_value(None, "user_version", |r| r.get(0))
}

/// Applies every migration newer than the database's `user_version`.
pub fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    migrate_to(conn, SCHEMA_VERSION)
}

/// Applies migrations up to `target`, each in its own transaction (tests use this to build
/// databases of an older version).
pub fn migrate_to(conn: &mut Connection, target: i64) -> rusqlite::Result<()> {
    let applied = usize::try_from(user_version(conn)?).unwrap_or(0);
    let target = usize::try_from(target).unwrap_or(0).min(MIGRATIONS.len());
    for (index, sql) in MIGRATIONS.iter().enumerate().take(target).skip(applied) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}
