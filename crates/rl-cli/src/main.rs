//! Developer CLI: record Rocket League sessions into an rl-trainer database.

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rl_app::devtools::{recording_script, FakeStatsApi};
use rl_app::paths::AppPaths;
use rl_app::platform::HeadlessPlatform;
use rl_app::settings::DEFAULT_PHONE_PORT;
use rl_app::{App, AppConfig};
use rl_core::facts::Fact;
use rl_core::fixture::open_spike_log;
use rl_core::ingest::{self, now_ms, IngestMsg, DEFAULT_ADDR};
use rl_core::pipeline::Pipeline;
use rl_core::store::{list_sessions, open_db, SessionRow, StoreError};
use rl_core::tracker::TrackerConfig;
use tokio::sync::{mpsc, watch};

const TICK_EVERY: Duration = Duration::from_secs(10);

#[derive(Parser)]
#[command(
    name = "rl-replay",
    about = "Record Rocket League sessions into an rl-trainer database"
)]
struct Cli {
    /// SQLite database file (created if missing). Required so a replay never lands in the app's
    /// own database by accident.
    #[arg(long)]
    db: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Feed a probe.mjs recording (.jsonl or .jsonl.gz) through the engine
    Replay { log: PathBuf },
    /// Print a replay file's MatchGUID and human players as JSON (end-to-end tests use it, so no
    /// account id is written into the tests)
    ReplayHeader { path: PathBuf },
    /// Listen to the running game until Ctrl+C
    Live {
        #[arg(long, default_value = DEFAULT_ADDR)]
        addr: String,
    },
    /// Print recorded sessions
    Sessions,
    /// Run the phone server without the desktop app (pair a phone against this database)
    Serve(ServeArgs),
}

#[derive(clap::Args)]
struct ServeArgs {
    /// Address to listen on; 127.0.0.1 keeps the server on this PC (tests)
    #[arg(long, default_value = "0.0.0.0")]
    host: IpAddr,
    #[arg(long, default_value_t = DEFAULT_PHONE_PORT)]
    port: u16,
    #[arg(long, default_value = DEFAULT_ADDR)]
    stats_addr: String,
    /// Replay a probe.mjs recording through a local fake Stats API instead of the game
    #[arg(long)]
    recording: Option<PathBuf>,
    /// Playback speed for --recording
    #[arg(long, default_value_t = 1.0)]
    speed: f64,
    /// Also serve the desktop page and its commands on 127.0.0.1:<PORT> (dev server, tests)
    #[arg(long)]
    desktop_port: Option<u16>,
    /// Watch this folder for saved replays instead of the game's folders
    #[arg(long)]
    replay_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Replay { log } => replay(&cli.db, &log)?,
        Command::ReplayHeader { path } => return replay_header(&path),
        Command::Live { addr } => live(&cli.db, addr).await?,
        Command::Sessions => {}
        Command::Serve(args) => return serve(&cli.db, args).await,
    }
    let conn = open_db(&cli.db).with_context(|| format!("opening {}", cli.db.display()))?;
    print_sessions(&list_sessions(&conn)?);
    Ok(())
}

fn replay(db: &Path, log: &Path) -> Result<()> {
    let events = open_spike_log(log).with_context(|| format!("reading {}", log.display()))?;
    let last = events.last().map_or(0, |e| e.at_ms);
    let mut pipeline = Pipeline::new(open_db(db)?, TrackerConfig::default())?;
    for event in events {
        print_facts(&pipeline.handle(&IngestMsg::Event(event))?);
    }
    print_facts(&pipeline.game_exited(last)?);
    Ok(())
}

async fn live(db: &Path, addr: String) -> Result<()> {
    let mut pipeline = Pipeline::new(open_db(db)?, TrackerConfig::default())?;
    let (tx, mut rx) = mpsc::channel(1024);
    let (stop_tx, stop_rx) = watch::channel(false);
    let ingest_task = tokio::spawn(ingest::run(addr.clone(), tx, stop_rx));
    let mut ticker = tokio::time::interval(TICK_EVERY);
    println!("Listening for Rocket League on {addr} (Ctrl+C to stop)");
    loop {
        tokio::select! {
            Some(msg) = rx.recv() => {
                match &msg {
                    IngestMsg::Connected { .. } => println!("connected"),
                    IngestMsg::Disconnected { .. } => println!("disconnected, retrying"),
                    IngestMsg::DecodeFailed { error, .. } => eprintln!("decode failed: {error}"),
                    IngestMsg::Event(_) => {}
                }
                report(pipeline.handle(&msg), &pipeline);
            }
            _ = ticker.tick() => report(pipeline.tick(now_ms()), &pipeline),
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    print_facts(&pipeline.game_exited(now_ms())?);
    stop_tx.send(true).ok();
    ingest_task.await?;
    Ok(())
}

fn replay_header(path: &Path) -> Result<()> {
    use rl_app::replays::extract::{human_keys, read_guid};
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let humans: Vec<String> = human_keys(&bytes)?.iter().map(|k| k.primary_id()).collect();
    println!(
        "{}",
        serde_json::json!({ "guid": read_guid(&bytes)?, "humans": humans })
    );
    Ok(())
}

async fn serve(db: &Path, args: ServeArgs) -> Result<()> {
    let ServeArgs {
        host,
        port,
        stats_addr,
        recording,
        speed,
        desktop_port,
        replay_dir,
    } = args;
    let fake = match &recording {
        Some(path) => {
            let script = recording_script(path, speed)
                .with_context(|| format!("reading {}", path.display()))?;
            Some(FakeStatsApi::start(script, None).await?)
        }
        None => None,
    };
    let addr = fake.as_ref().map_or(stats_addr, |f| f.addr.clone());
    let config = AppConfig {
        stats_addr_override: Some(addr.clone()),
        poll_every: Duration::from_secs(1),
        replay_dir,
        ..AppConfig::new(AppPaths::for_db(db))
    };
    let app = App::start(config, Arc::new(HeadlessPlatform)).await?;
    let pairing = app.enable_phone_at(SocketAddr::new(host, port)).await?;
    println!("Stats API: {addr}");
    println!("Open on your phone: {}", pairing.url);
    for other in &pairing.addresses {
        println!("  (other addresses: {other})");
    }
    match rl_app::pairing::qr_text(&pairing.url) {
        Ok(qr) => println!("{qr}"),
        Err(err) => eprintln!("could not draw the QR code: {err}"),
    }
    println!("The firewall is not configured in serve mode: allow rl-replay on private networks if Windows asks.");
    let _desktop = match desktop_port {
        Some(port) => {
            let (addr, task) = rl_app::desktop::http::start(app.clone(), port).await?;
            println!("Desktop page: http://{addr}/");
            Some(task)
        }
        None => None,
    };
    println!("Serving until Ctrl+C.");
    tokio::signal::ctrl_c().await?;
    app.shutdown().await;
    Ok(())
}

/// A failed save keeps the facts queued in the pipeline, so live recording carries on.
fn report(result: Result<Vec<Fact>, StoreError>, pipeline: &Pipeline) {
    match result {
        Ok(facts) => print_facts(&facts),
        Err(err) => eprintln!(
            "could not save yet ({err}); {} fact(s) queued for retry",
            pipeline.pending_facts()
        ),
    }
}

fn print_facts(facts: &[Fact]) {
    for fact in facts {
        println!("{fact:?}");
    }
}

fn print_sessions(sessions: &[SessionRow]) {
    for s in sessions {
        let minutes = s.ended_at_ms.map_or("open".to_string(), |end| {
            format!("{} min", (end - s.started_at_ms) / 60_000)
        });
        println!("Session #{} ({minutes})", s.id);
        for b in &s.blocks {
            let detail = match (b.make_rate(), &b.match_result) {
                (Some(rate), _) => format!(
                    "{} attempts, {} makes ({:.0}%)",
                    b.attempts,
                    b.makes,
                    rate * 100.0
                ),
                (None, Some(result)) => result.clone(),
                (None, None) => String::new(),
            };
            let playlist = b.playlist_id.map_or("-".into(), |p| p.to_string());
            println!("  - {:<8} playlist {playlist:<4} {detail}", b.kind);
        }
    }
}
