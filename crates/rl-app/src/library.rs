//! What the desktop window reads and edits. Reads use their own connection; every edit runs on
//! the engine (the only writer), and merge, split and delete take a backup first.

use rl_core::coach::review::SessionReview;
use rl_core::ingest::now_ms;
use rl_core::store::edits::{EditError, MmrInput, PackStats};
use rl_core::store::packs::{self, PackDetail, PackInput, PackSort};
use rl_core::store::progress::{
    self, ModeProgress, OverviewProgress, PackProgress, ProgressList, ProgressRange,
};
use rl_core::store::replays::{self, ReplayToSave};
use rl_core::store::review;
use rl_core::store::session_edits;
use rl_core::store::sessions::{self, Neighbour, SessionCard, SessionDetail};
use rusqlite::Connection;

use crate::app::{App, AppError};
use crate::backup;
use crate::RELEASES_URL;

/// Which edit a before-edit backup protects (it names the file).
#[derive(Debug, Clone, Copy)]
enum EditKind {
    Merge,
    Split,
    Delete,
}

impl EditKind {
    fn action(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Split => "split",
            Self::Delete => "delete",
        }
    }
}

impl App {
    pub fn session_cards(
        &self,
        search: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<SessionCard>, AppError> {
        Ok(sessions::session_cards(
            &*self.read_db()?,
            search,
            offset,
            limit,
        )?)
    }

    pub fn review_count(&self) -> Result<i64, AppError> {
        Ok(sessions::review_count(&*self.read_db()?)?)
    }

    pub fn session_detail(&self, id: i64) -> Result<Option<SessionDetail>, AppError> {
        Ok(sessions::session_detail(&*self.read_db()?, id)?)
    }

    pub fn session_review(&self, id: i64) -> Result<Option<SessionReview>, AppError> {
        Ok(review::session_review(&*self.read_db()?, id, now_ms())?)
    }

    pub fn match_replay(&self, match_id: i64) -> Result<crate::desktop::MatchReplay, AppError> {
        use rl_core::coach::{benchmark, insights};
        let conn = self.read_db()?;
        let players = replays::players_for_match(&conn, match_id)?;
        let (matches, pool) = replays::benchmark_pool(&conn, match_id)?;
        let team_size = replays::match_team_size(&conn, match_id)?.unwrap_or(1);
        let me: Vec<_> = players
            .iter()
            .filter(|p| p.is_me)
            .map(|p| &p.stats)
            .collect();
        let bench = benchmark::benchmark(&pool, matches, &me);
        let found =
            insights::insights(&players, &benchmark::winner_rows(&pool, matches), team_size);
        Ok(crate::desktop::MatchReplay {
            players,
            benchmark: bench,
            insights: found,
        })
    }

    /// Ranked matches whose replay is still to save (the badge and the list lane).
    pub fn replays_to_save(&self) -> Result<Vec<ReplayToSave>, AppError> {
        Ok(replays::to_save(&*self.read_db()?, now_ms())?
            .into_iter()
            .filter(|m| m.ranked)
            .collect())
    }

    pub fn session_cards_to_save(&self) -> Result<Vec<SessionCard>, AppError> {
        let mut ids: Vec<i64> = self
            .replays_to_save()?
            .iter()
            .map(|m| m.session_id)
            .collect();
        ids.dedup();
        Ok(sessions::session_cards_in(&*self.read_db()?, &ids)?)
    }

    pub fn replay_status(&self) -> Result<crate::desktop::ReplayStatus, AppError> {
        Ok(crate::desktop::ReplayStatus {
            folders: self
                .replay_dirs()?
                .iter()
                .map(|d| d.display().to_string())
                .collect(),
            counts: replays::counts(&*self.read_db()?)?,
        })
    }

    pub fn pack_list(&self, sort: PackSort, search: &str) -> Result<Vec<PackStats>, AppError> {
        Ok(packs::pack_list(&*self.read_db()?, sort, search)?)
    }

    pub fn pack_detail(&self, id: i64) -> Result<Option<PackDetail>, AppError> {
        Ok(packs::pack_detail(&*self.read_db()?, id)?)
    }

    pub fn progress_list(&self, range: ProgressRange) -> Result<ProgressList, AppError> {
        Ok(progress::progress_list(&*self.read_db()?, range, now_ms())?)
    }

    pub fn progress_overview(&self, range: ProgressRange) -> Result<OverviewProgress, AppError> {
        Ok(progress::progress_overview(
            &*self.read_db()?,
            range,
            now_ms(),
        )?)
    }

    pub fn progress_mode(
        &self,
        team_size: u8,
        range: ProgressRange,
    ) -> Result<ModeProgress, AppError> {
        Ok(progress::progress_mode(
            &*self.read_db()?,
            team_size,
            range,
            now_ms(),
        )?)
    }

    /// My last ranked matches in a mode against their winners (progress radar spec §3).
    pub fn mode_benchmark(
        &self,
        team_size: u8,
    ) -> Result<rl_core::coach::benchmark::Benchmark, AppError> {
        let conn = self.read_db()?;
        let (matches, pool) = replays::recent_pool(&conn, i64::from(team_size))?;
        let me: Vec<_> = pool.iter().filter(|r| r.is_me).map(|r| &r.stats).collect();
        Ok(rl_core::coach::benchmark::benchmark(&pool, matches, &me))
    }

    pub fn progress_pack(
        &self,
        pack_id: i64,
        range: ProgressRange,
    ) -> Result<Option<PackProgress>, AppError> {
        Ok(progress::progress_pack(
            &*self.read_db()?,
            pack_id,
            range,
            now_ms(),
        )?)
    }

    pub fn open_data_folder(&self) -> Result<(), AppError> {
        Ok(self.inner.platform.open_folder(&self.inner.paths.root)?)
    }

    /// Settings → About → Releases. A fixed address, so the page can't open arbitrary URLs.
    pub fn open_releases(&self) -> Result<(), AppError> {
        Ok(self.inner.platform.open_url(RELEASES_URL)?)
    }

    async fn edit<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Connection) -> Result<T, EditError> + Send + 'static,
    ) -> Result<T, AppError> {
        Ok(self
            .engine()
            .await?
            .run(move |pipeline| pipeline.with_conn(now_ms(), f))
            .await?)
    }

    /// Checks the edit, backs the database up, then applies it — all on the engine, so nothing
    /// is recorded in between. A refused edit takes no backup.
    async fn guarded_edit<P: Send + 'static, T: Send + 'static>(
        &self,
        kind: EditKind,
        plan: impl FnOnce(&Connection) -> Result<P, EditError> + Send + 'static,
        apply: impl FnOnce(&Connection, P) -> Result<T, EditError> + Send + 'static,
    ) -> Result<T, AppError> {
        let backups = self.inner.paths.backups.clone();
        self.edit(move |conn| {
            let planned = plan(conn)?;
            backup::before_edit(conn, &backups, kind.action()).map_err(|err| {
                tracing::error!(error = %err, "before-edit backup failed; the edit was not made");
                EditError::Conflict(format!("the backup before this change failed: {err}"))
            })?;
            apply(conn, planned)
        })
        .await
    }

    pub async fn edit_review(
        &self,
        session_id: i64,
        mmr: Vec<MmrInput>,
        notes: String,
    ) -> Result<(), AppError> {
        self.edit(move |c| session_edits::edit_review(c, session_id, &mmr, &notes, now_ms()))
            .await
    }

    pub async fn merge_sessions(&self, session_id: i64, with: Neighbour) -> Result<i64, AppError> {
        self.guarded_edit(
            EditKind::Merge,
            move |c| session_edits::plan_merge(c, session_id, with),
            |c, plan| session_edits::apply_merge(c, plan, now_ms()),
        )
        .await
    }

    pub async fn split_session(
        &self,
        session_id: i64,
        after_block_id: i64,
    ) -> Result<i64, AppError> {
        self.guarded_edit(
            EditKind::Split,
            move |c| session_edits::plan_split(c, session_id, after_block_id),
            |c, plan| session_edits::apply_split(c, plan, now_ms()),
        )
        .await
    }

    pub async fn delete_session(&self, session_id: i64) -> Result<(), AppError> {
        self.guarded_edit(
            EditKind::Delete,
            move |c| session_edits::check_delete_session(c, session_id),
            move |c, ()| session_edits::apply_delete_session(c, session_id),
        )
        .await
    }

    /// Returns whether the session went with its last activity.
    pub async fn delete_activity(&self, block_id: i64) -> Result<bool, AppError> {
        self.guarded_edit(
            EditKind::Delete,
            move |c| session_edits::check_delete_activity(c, block_id),
            move |c, session_id| session_edits::apply_delete_activity(c, block_id, session_id),
        )
        .await
    }

    pub async fn add_pack(&self, pack: PackInput) -> Result<i64, AppError> {
        self.edit(move |c| packs::add_pack(c, &pack)).await
    }

    pub async fn update_pack(&self, id: i64, pack: PackInput) -> Result<(), AppError> {
        self.edit(move |c| packs::update_pack(c, id, &pack)).await
    }

    pub async fn set_pack_hidden(&self, id: i64, hidden: bool) -> Result<(), AppError> {
        self.edit(move |c| packs::set_pack_hidden(c, id, hidden))
            .await
    }
}
