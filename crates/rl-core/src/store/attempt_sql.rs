//! The make-rate rule in SQL (spec §2). Every query that counts attempts or makes uses these
//! fragments, so the rule lives in one place (with `coach::rules` for in-memory attempts).

use crate::coach::rules::QUICK_RESET_MS;

/// True for an attempt of alias `a` that counts: not skipped, and not a quick reset outside a
/// goalie pack. `a` is a fixed alias from our own SQL, never user input.
pub(crate) fn counted(a: &str) -> String {
    format!(
        "({a}.skipped = 0 AND ({a}.outcome = 'make' OR {a}.duration_ms >= {QUICK_RESET_MS}
           OR EXISTS (SELECT 1 FROM block qb JOIN pack qp ON qp.id = qb.pack_id
                      WHERE qb.id = {a}.block_id AND qp.goalie = 1)))"
    )
}

/// True for a made attempt of alias `a`.
pub(crate) fn made(a: &str) -> String {
    format!("{a}.outcome = 'make'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::open_in_memory;
    use rusqlite::Connection;

    fn seed(conn: &Connection, goalie: bool) {
        conn.execute_batch(&format!(
            "UPDATE pack SET goalie = {} WHERE id = 1;
             INSERT INTO session (id, started_at, last_seen_at) VALUES (1, 0, 0);
             INSERT INTO block (id, session_id, kind, pack_id, started_at) VALUES (1, 1, 'training', 1, 0);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome, skipped) VALUES
               (1, 1, 0, 1200, 'miss', 0),
               (1, 2, 1, 2500, 'miss', 0),
               (1, 3, 2, 900, 'make', 0),
               (1, 4, 3, 5000, 'make', 1);",
            goalie as i32
        ))
        .unwrap();
    }

    fn rate(conn: &Connection) -> (i64, i64) {
        let sql = format!(
            "SELECT COUNT(*), COALESCE(SUM({}), 0) FROM training_attempt a WHERE {}",
            made("a"),
            counted("a")
        );
        conn.query_row(&sql, [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
    }

    #[test]
    fn quick_resets_and_skips_are_left_out() {
        let conn = open_in_memory().unwrap();
        seed(&conn, false);
        assert_eq!(
            rate(&conn),
            (2, 1),
            "the 1.2 s miss and the skipped make are left out"
        );
    }

    #[test]
    fn goalie_packs_count_fast_misses() {
        let conn = open_in_memory().unwrap();
        seed(&conn, true);
        assert_eq!(rate(&conn), (3, 1));
    }

    #[test]
    fn no_query_counts_makes_outside_the_shared_rule() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/store");
        let mut offenders = Vec::new();
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.file_name().unwrap() != "attempt_sql.rs" {
                    let text = std::fs::read_to_string(&path).unwrap();
                    // Production SQL only: the tests module may seed with any text.
                    let code = text.split("#[cfg(test)]").next().unwrap();
                    if code.contains("outcome = 'make'") || code.contains("skipped = 0") {
                        offenders.push(path.display().to_string());
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "use attempt_sql::counted/made in: {offenders:?}"
        );
    }
}
