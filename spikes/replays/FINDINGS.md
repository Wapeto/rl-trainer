# Replay-file spike: findings

- **Date:** 2026-10-12
- **Question:** can rl-trainer get positioning, rotation and boost-pad data from replay files, safely and without extra work for the player?
- **Probe:** `src/main.rs` (throwaway; kept out of the main workspace). Commands:
  - `replay-probe header <file>`: header properties;
  - `replay-probe scan <dir>`: parse every replay in a folder, with network frames;
  - `replay-probe stats <file>`: subtr-actor's full stats pass, as JSON;
  - `replay-probe boost <file> <epic-id>`: the player's boost per frame and touch frames.
- No replay files or outputs are committed: they hold other players' names and IDs.

## 1. Where the replays are

- Folder: `%USERPROFILE%\OneDrive\Documents\My Games\Rocket League\TAGame\DemosEpic`. Documents is redirected to OneDrive, so `%USERPROFILE%\Documents\…` does not exist on this PC. `Demos` (Steam) holds 141 older files.
- 2,510 replays. They were saved at 76–261 a month from Oct 2023 to Nov 2024, then the saving stopped. Only 8 were saved after that, the newest on 2026-09-14.
- None of the 21 matches rl-trainer recorded on 7–12 Oct 2026 has a replay.
- **So:** the game does not save replays by itself here. The 2023–24 rate looks like an auto-saver plugin (BakkesMod), which stopped with EAC.

## 2. Parsing (boxcars 0.12.0)

- 2,506 of 2,510 replays parse with network data.
  - The 4 failures are corrupt files of under 5 KB, all from 2024-10-30.
  - 23 old files have no `BuildVersion` and 0 frames.
- The current game build parses: 260811 and 260825 (Aug–Sep 2026, net version 11), and 261001 (replays saved 2026-10-12: version 868.34, net version 12).
- Speed: 4–12 ms per replay for the boxcars parse alone (release build).
- Header fields useful for rl-trainer: `MatchGUID`, `Date`, `TeamSize`, `MatchType` (Online, Tournament, …), `PlayerStats` (with each player's Epic/Steam ID), `Goals` with frames, and `TotalSecondsPlayed`.

## 3. Matching a replay to a recorded match

- `MatchGUID` in the header has the same 32-hex format as `match_record.match_guid`, which comes from the Stats API.
- The replay's player IDs use the same Epic account ID as `profile.primary_id` (`Epic|<id>|0`). "Me" is found by ID, not by name.
- **Confirmed on 2026-10-12:** the user saved 4 replays of matches rl-trainer had recorded (8, 9 and 12 Oct). All 4 `MatchGUID`s equal `match_record.match_guid`. Goals, saves and shots in the replay header equal the recorded values in all 4.
- The header `Date` is not local time (a match recorded starting at 18:33 local has `Date` 16-39-35), so match on the GUID, never on the date.

## 4. What the data gives (subtr-actor 1.4.0)

subtr-actor's `StatsCollector` runs about 60 stat modules in one pass. It took 2.2 s for a 5-minute 1v1 and 5.2 s for a 2v2, in release. The ones that matter for this project:

| Module | Per player | Example (my Sep 14 1v1 / Aug 31 2v2) |
|---|---|---|
| `positioning` | time behind / in front of / level with the ball, by half and by third, most back / most forward, shadow defence, distance to ball and teammates | 1v1: 204 s behind the ball and 71 s in front of it, out of 305 s |
| `rotation` (2v2/3v3) | time as 1st / 2nd / 3rd man, became first man count, longest first-man stint, role events | 2v2: 140 s first man, 139 s second man, became first man 30 times |
| `boost` | collected, used, stolen, big and small pads, each pickup with pad, position and boost before/after, used while airborne or supersonic | 1v1: 13 big pads, 2 stolen |
| `fifty_fifty`, `kickoff` | wins, losses, neutral; kickoff approach and outcome | 1v1: kickoffs 0–2, open-play 50/50s 4–2 |
| `whiff` | missed touches, aerial or grounded, with closest approach | 1v1: 9 aerial whiffs |
| `touch` | touches by kind (control, hard hit, aerial, first touch), action (shot, clear, save, boom) | |
| `movement`, `demo`, `powerslide`, mechanics (flicks, wavedashes, …) | | |

**"Commits without boost"** (the user's own note, 8 Oct) can be computed: the player's boost per frame (`ReplayDataCollector`), joined to the attributed touch and 50/50 events by frame:

- Sep 14 1v1: 15 of 36 touches with under 20 boost (8 at 0); 3 of 6 open-play 50/50s started under 20 boost.
- Aug 31 2v2: 12 of 33 touches under 20 boost (6 at 0); no 50/50 under 20.

**Care needed:** boost amounts mix units (raw 0–255 and percent) between modules; subtr-actor's `boost_units` must be followed.

## 5. Cost and safety

- Both crates are MIT. The probe's lockfile pulls in 57 crates; the release exe is 4.3 MB.
- Reading replay files from disk is passive. It never touches the game process, so it is EAC-safe.
- The stats pass takes seconds, so it should run on a background thread after the match, never in the live pipeline.

## 6. Saving replays

- Replays can be saved **after the fact from the in-game match history screen**: on 12 Oct the user saved replays of matches played on 8, 9 and 12 Oct. No click at the end of each match is needed.
- The game has **no auto-save setting** (checked in game by the user, 2026-10-12). Saving is manual: at match end, or later from match history.
- Match history keeps **the last 20 online matches from the past 30 days** (in-game note). Private matches count towards the 20. At the user's pace (11 matches on 8 Oct alone), a match can drop out of history within a few days.
- Results on the 4 new replays (stats pass 3–6 s each):

| Match | Result | Behind ball | 1st / 2nd man | 50/50 W–L | Whiffs | Touches under 20 boost |
|---|---|---|---|---|---|---|
| 2v2, 8 Oct | win 3–1 | 68% | 137 s / 133 s | 4–1 | 1 | 7 of 33 |
| 2v2, 8 Oct | loss 5–6 | 66% | 193 s / 144 s | 5–4 | 7 | 17 of 36 |
| 2v2, 9 Oct | loss 0–3 | 63% | 131 s / 147 s | 3–2 | 2 | 9 of 21 |
| 1v1, 12 Oct | loss 6–7 | 54% | — | 15–6 | 7 | 33 of 67 |

## 7. Recommendation

**Go: replay stats are worth a place in Coach v1.**

1. rl-trainer watches the `DemosEpic` folder (found from the Documents known-folder path, so the OneDrive redirect is handled). A new `.replay` file is parsed on a background thread with boxcars and subtr-actor. It is tied to its match by `MatchGUID`, and "me" is found by `profile.primary_id`.
2. A small per-match table stores the few stats a coach needs, not the whole stats output:
   - positioning: behind / in front of the ball, by third;
   - rotation: 1st / 2nd / 3rd man time, became-first-man count;
   - boost: big and small pads, stolen pads, touches and 50/50s under 20 boost;
   - 50/50 and kickoff wins and losses, whiffs.
3. A replay is optional. Every match keeps its live-feed stats; the replay only adds to it. Because saving is manual and history is short, the app should list recorded matches that have no replay and are still in history (newest 20 online matches, under 30 days old), with the oldest flagged as about to drop out.
4. Pin the subtr-actor version. Its outputs are large and change between releases, and boost units differ between modules, so the stored stats need tests against a few fixture replays (anonymized, or generated, not committed with real names).

## Header for results and backfill

Checked on 2026-10-21 against the newest replays in the local Demos folder (nothing committed).

- `Team0Score` / `Team1Score`: present, but **left out when the score is 0** (read a missing one as 0).
- `PrimaryPlayerTeam`: the saving player's team; **left out when it is 0**. Prefer the team of the player marked `is_me` in the stats pass.
- `PlayerStats[]`: `Name`, `Platform`, `Team`, `Score`, `Goals`, `Assists`, `Saves`, `Shots`, `bBot`. `OnlineID` can be `"0"` for Epic players, so match "me" through the stats pass (`remote_id`), not the header.
- `TotalSecondsPlayed` (regulation is about 300 s), `Date` (`YYYY-MM-DD HH-MM-SS`, UTC), `TeamSize`, `MatchType` (`Online`, `Private`, `FTE`, ...).
- **No** overtime flag, forfeit flag, playlist id or ranked flag. `MatchType = Online` covers casual and ranked alike.
- Consequences: an imported game gets `overtime = false`, `ranked = false` (spec §2.2 fallback), the playlist is guessed from `TeamSize` only when unambiguous, and only `Online` replays are imported. A live record keeps its own overtime flag when a replay overrides the score.
