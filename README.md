<p align="center"><img src="apps/desktop/src-tauri/icons/128x128.png" width="96" alt="RL Trainer logo"></p>

# RL Trainer

A training logbook for Rocket League on Windows. It records your free play, training packs and
ranked matches on its own, shows your progress over time, and points out what stood out in each
session. A phone page on your Wi-Fi lets you log reps and see stats without alt-tabbing.

It only reads the game's official **Stats API** and your saved replays. No overlays, no
injection, nothing that touches the game process, so it is safe with Easy Anti-Cheat.

> Not affiliated with or endorsed by Psyonix or Epic Games. Rocket League is a trademark of
> Psyonix LLC.

## Install

1. Download `RL Trainer_<version>_x64-setup.exe` from the
   [latest release](https://github.com/Wapeto/rl-trainer/releases/latest).
2. Run it. Windows may show **"Windows protected your PC"** because the installer is not
   code-signed: click **More info → Run anyway**. No administrator rights are needed.
3. Start RL Trainer. It lives in the tray and keeps recording when the window is closed.

Updates: the app checks for a new version shortly after it starts and shows a banner. Nothing
installs until you click **Install and restart**.

## First start

The **Get started** card on the Sessions page walks you through it:

1. **Stats API.** The game ships with it turned off. Click **Fix…** (asks for administrator
   rights once), then restart Rocket League. A game update can turn it off again; the app checks
   every time the game starts.
2. **Your account.** Open free play once and the app picks you up. Or enter your in-game name
   (or Epic account ID) in **Settings → Tracking**. Steam accounts work too.
3. **Phone (optional).** Turn on the phone page in Settings, allow the firewall prompt, and scan
   the QR code with a phone on the same Wi-Fi. Your network must be set to *Private* in Windows.
   Another firewall (for example Portmaster) may block RL Trainer even after Windows allows it.

## Your data

Everything stays on your PC in `%LOCALAPPDATA%\rl-trainer\`: the database, daily backups (the
last 7 are kept) and logs. There are no accounts and nothing is uploaded, except a report you
choose to send.

## Feedback

Use the **Feedback** page in the app (bottom of the sidebar). Pick Bug or Idea, describe it, and
paste a screenshot with Ctrl+V if it helps. The page lists exactly what gets sent. If sending
fails, **Save report** writes the files to `Documents\RL Trainer reports\` so you can send them
another way.

## Building from source

Needs Rust (stable, MSVC toolchain) and Node.js 22.

```bash
cargo run -p rl-trainer
```

`cargo build` also builds the UI in `ui/` (it runs `npm ci` the first time). Tests:

```bash
cargo test --workspace
```

```bash
npm --prefix ui test
```

Installer: `npx @tauri-apps/cli build` in `apps/desktop/src-tauri`. Release builds also sign the
update files, so they need `TAURI_SIGNING_PRIVATE_KEY` (and its password) set. Without the key,
build with `--config "{\"bundle\":{\"createUpdaterArtifacts\":false}}"`. Releases are built by
GitHub Actions when a `v*` tag is pushed.

The feedback relay (a small Cloudflare Worker) is in `tools/report-relay/`.

## License

[MIT](LICENSE)
