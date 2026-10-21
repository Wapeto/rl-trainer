# Changelog

## 0.2.2

- **Replays are the source of truth.** When a replay is imported, its final score replaces what
  the app recorded live, so a game you lost after a crash or disconnect no longer shows as a win.
- **Import past games** (Settings → Replays): adds your 20 newest online games from your replay
  folder, even ones played before you installed the app. New installs do it once on their own.
  Imported days are marked "Imported from replays"; they count as casual, since replays don't
  say whether a game was ranked.
- **Pair a phone faster:** click the phone line at the bottom of the sidebar for the QR code.
- **Install counter:** once a day the app sends a random install ID and its version, nothing
  else. Turn it off in Settings → App → Count this install.
- Fixes: the Stats API is checked when the app starts (no more "Check again" after every start);
  a phone firewall rule left from another copy of the app is repaired; deleted matches stay
  deleted.

## 0.2.1

- After **Fix…** for the Stats API, the app now says to restart Rocket League: the game only
  reads that setting when it starts. While the game runs without sending data, Settings says so.
- Settings → Phone explains that Windows must treat your Wi-Fi as Private for the phone to connect.

## 0.2.0 — first release for testers

- Records free play, training packs and ranked/casual matches from the Stats API, and imports
  saved replays for per-match stats.
- Sessions, Packs and Progress pages; coach review after each session; winners benchmark.
- Phone page on your Wi-Fi for logging reps and MMR.
- Set your account by in-game name, Epic or Steam id (Settings → Tracking).
- Get started checklist for a new install.
- Installer, and in-app updates from GitHub Releases.
- Feedback page: send a bug or an idea with a screenshot and recent logs.
