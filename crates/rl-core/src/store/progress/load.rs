//! Reads shared by several progress views.

use rusqlite::{params, Connection, OptionalExtension};

use super::{MmrPoint, SessionTime};
use crate::store::attempt_sql::counted;
use crate::store::edits::{pack, PackRow};

/// MMR entries of a team size in the range (oldest first) and the last one before it.
pub(crate) fn mmr_points(
    conn: &Connection,
    team_size: u8,
    start: i64,
) -> rusqlite::Result<(Option<i64>, Vec<MmrPoint>)> {
    let before = conn
        .query_row(
            "SELECT mmr FROM mmr_entry WHERE team_size = ?1 AND recorded_at < ?2
             ORDER BY recorded_at DESC, id DESC LIMIT 1",
            params![team_size, start],
            |r| r.get(0),
        )
        .optional()?;
    let mut stmt = conn.prepare(
        "SELECT session_id, recorded_at, mmr FROM mmr_entry
         WHERE team_size = ?1 AND recorded_at >= ?2 ORDER BY recorded_at, id",
    )?;
    let points = stmt
        .query_map(params![team_size, start], |r| {
            Ok(MmrPoint {
                session_id: r.get(0)?,
                at: r.get(1)?,
                mmr: r.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok((before, points))
}

/// Every session of the range with its finished play time by kind, oldest first. Activities
/// still running count from the next read on.
pub(crate) fn session_times(conn: &Connection, start: i64) -> rusqlite::Result<Vec<SessionTime>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.started_at, b.kind, COALESCE(b.ended_at - b.started_at, 0),
                EXISTS (SELECT 1 FROM match_record m WHERE m.block_id = b.id AND m.ranked = 1)
         FROM session s
         LEFT JOIN block b ON b.session_id = s.id AND b.ended_at IS NOT NULL
         WHERE s.started_at >= ?1
         ORDER BY s.started_at, s.id",
    )?;
    let mut rows = stmt.query(params![start])?;
    let mut out: Vec<SessionTime> = Vec::new();
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        if out.last().map(|t| t.session_id) != Some(id) {
            out.push(SessionTime {
                session_id: id,
                at: r.get(1)?,
                ..SessionTime::default()
            });
        }
        let kind: Option<String> = r.get(2)?;
        let ms: i64 = r.get(3)?;
        let ranked: bool = r.get(4)?;
        if let Some(t) = out.last_mut() {
            match kind.as_deref() {
                Some("training") => t.training_ms += ms,
                Some("freeplay") => t.freeplay_ms += ms,
                Some("match") if ranked => t.ranked_ms += ms,
                Some(_) => t.other_ms += ms,
                None => {}
            }
        }
    }
    Ok(out)
}

/// Packs with at least one counted attempt in the range (hidden ones too), by name.
pub(crate) fn played_packs(conn: &Connection, start: i64) -> rusqlite::Result<Vec<PackRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT DISTINCT b.pack_id FROM block b
         JOIN session s ON s.id = b.session_id
         JOIN training_attempt a ON a.block_id = b.id AND {}
         WHERE b.pack_id IS NOT NULL AND s.started_at >= ?1",
        counted("a")
    ))?;
    let ids: Vec<i64> = stmt
        .query_map(params![start], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut packs = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(p) = pack(conn, id)? {
            packs.push(p);
        }
    }
    packs.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(packs)
}
