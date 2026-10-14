# Stats API spike — findings (2026-10-07)

Captured with `probe.mjs`, `PacketSendRate=10`, Epic build, EAC on. ~15 min: menu, freeplay, custom training, casual 2v2, saved replay.

## Mode detection (`UpdateState.Game.PlaylistId`)

| Observed | PlaylistId | Arena example | Notes |
|---|---|---|---|
| Free play | 9 | `CHN_Stadium_P` | `TimeSeconds` counts **up** |
| Custom training | 21 | `Stadium_P` | `TimeSeconds` = per-shot countdown starting at 10 |
| Casual doubles | 2 | `NeoTokyo_Arcade_P` | `MatchGuid` populated only for online matches |
| Saved replay (ranked 3v3) | 13 | `UF_Day_P` | `ReplayCreated` event; extra `Game.Frame`, `Game.Elapsed` |

| Ranked 1v1 | 10 | `UtopiaStadium_Dusk_P` | confirmed in the evening recording |
| Ranked 2v2 | 11 | `UtopiaStadium_P` | confirmed in the evening recording |

Menu emits nothing. Still unmapped: casual 1v1/3v3, extra modes, offline bots.

## Evening recording (labelled training behaviour)

- **Timeout:** the timer reaches 0, then the game emits a `GoalScored` with an **empty `Scorer.Name`** and a slow `GoalSpeed` (~11–14) — the shot-ending explosion, not a goal. Blue score does not change. Real makes always have the player as `Scorer`.
- **Reset after a miss:** `CountdownBegin` + `RoundStarted` with time left (5 s in the test).
- **Retry after a make:** `CountdownBegin` and `RoundStarted` arrive together; Blue `Score` drops back by 1.
- **Skip:** mostly `CountdownBegin` without `RoundStarted`; a shot skipped after it started looks like a reset.
- **Waiting in the preview:** after `CountdownBegin` the timer shows 10 again until the player starts the shot, so an attempt must close at `CountdownBegin` and judge timeouts by the lowest timer value seen.

## Custom training is auto-trackable (partially)

- Every attempt = `CountdownBegin` + `RoundStarted`; timer resets to 10.
- A made shot = `GoalScored` (+ Blue `Teams[0].Score` increments).
- Retrying a shot after scoring **decrements** the team score.
- **Not exposed:** pack code/name, shot index, or the difference between miss / reset / skip.
  → The phone tells the app which pack is active; the app counts attempts and goals itself; taps are only needed to annotate (skip/reset) if we want per-shot accuracy.

## Free play

Goals, `BallHit` with `PreHitSpeed`/`PostHitSpeed` + location (power-shot tracking), crossbar hits, own car speed/boost/supersonic/on-wall/on-ground at the send rate.

## Matches

- Per-player scoreboard (score, goals, shots, assists, saves, touches, car touches, demos) for all players.
- Own `Boost`, `Speed`, `bSupersonic`, `bBoosting`, `bOnWall`, `bPowersliding` present; opponents' boost/flags absent.
- `StatfeedEvent` types seen: Shot, Goal, Assist, Save, EpicSave, Demolish, BicycleHit, BicycleGoal, OvertimeGoal, Win, MVP.
- `MatchEnded.WinnerTeamNum` gives the result.
- **Quirk:** goal replays re-emit `GoalScored` with `GoalSpeed: 0` — ignore `GoalScored` between `GoalReplayStart` and `GoalReplayEnd`.
- **No MMR / rank** anywhere in the feed.

## Practical notes

- Own identity: `PrimaryId` `Epic|<id>|0`; display name casing varies (`Wapeto` vs `wapeto`) — key on `PrimaryId`.
- Raw log was 19 MB for 15 min at 10 Hz — the app must aggregate `UpdateState`, not store it raw.
