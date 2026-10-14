//! Spike probe: parse Rocket League replays with boxcars and report what they hold.
//! Usage: replay-probe header <file> | replay-probe scan <dir>
use std::{env, fs, path::Path, time::Instant};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("header") => header(Path::new(&args[2])),
        Some("scan") => scan(Path::new(&args[2])),
        Some("stats") => stats(Path::new(&args[2])),
        Some("boost") => boost(Path::new(&args[2]), &args[3]),
        _ => anyhow::bail!("usage: replay-probe header <file> | scan <dir>"),
    }
}

fn parse(path: &Path) -> anyhow::Result<boxcars::Replay> {
    let data = fs::read(path)?;
    Ok(boxcars::ParserBuilder::new(&data).must_parse_network_data().parse()?)
}

fn header(path: &Path) -> anyhow::Result<()> {
    let r = parse(path)?;
    println!("versions: {}.{} net {:?}", r.major_version, r.minor_version, r.net_version);
    for (k, v) in &r.properties {
        let s = serde_json::to_string(v)?;
        println!("{k}: {}", if s.len() > 300 { &s[..300] } else { &s });
    }
    let frames = r.network_frames.as_ref().map_or(0, |f| f.frames.len());
    println!("frames: {frames}, objects: {}", r.objects.len());
    Ok(())
}

fn prop<'a>(r: &'a boxcars::Replay, key: &str) -> Option<&'a boxcars::HeaderProp> {
    r.properties.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn scan(dir: &Path) -> anyhow::Result<()> {
    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "replay"))
        .map(|e| (e.metadata().and_then(|m| m.modified()).ok(), e.path()))
        .collect();
    files.sort();
    let (mut ok, mut bad) = (0, 0);
    for (_, path) in &files {
        let t = Instant::now();
        let name = path.file_name().unwrap().to_string_lossy();
        match parse(path) {
            Ok(r) => {
                ok += 1;
                let build = prop(&r, "BuildVersion").and_then(|p| p.as_string()).unwrap_or("?");
                let date = prop(&r, "Date").and_then(|p| p.as_string()).unwrap_or("?");
                let frames = r.network_frames.as_ref().map_or(0, |f| f.frames.len());
                println!("OK  {name} {date} build={build} net={:?} frames={frames} {}ms", r.net_version, t.elapsed().as_millis());
            }
            Err(e) => {
                bad += 1;
                println!("ERR {name} {e}");
            }
        }
    }
    println!("total {} ok {ok} err {bad}", files.len());
    Ok(())
}

fn stats(path: &Path) -> anyhow::Result<()> {
    let r = parse(path)?;
    let t = Instant::now();
    let collected = subtr_actor::StatsCollector::new()
        .get_stats(&r)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    eprintln!("stats pass: {}ms", t.elapsed().as_millis());
    println!("{}", serde_json::to_string_pretty(&collected)?);
    Ok(())
}

/// My boost (0-100) per frame and my touch frames, as JSON. `epic` is my Epic account id.
fn boost(path: &Path, epic: &str) -> anyhow::Result<()> {
    let r = parse(path)?;
    let data = subtr_actor::ReplayDataCollector::new()
        .get_replay_data(&r)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let is_me = |id: &boxcars::RemoteId| matches!(id, boxcars::RemoteId::Epic(e) if e == epic);
    let (_, me) = data
        .frame_data
        .players
        .iter()
        .find(|(id, _)| is_me(id))
        .ok_or_else(|| anyhow::anyhow!("player not in replay"))?;
    let boost: Vec<Option<f32>> = me
        .frames()
        .iter()
        .map(|f| match f {
            subtr_actor::PlayerFrame::Data { boost_amount, .. } => Some(boost_amount / 2.55),
            _ => None,
        })
        .collect();
    let touches: Vec<usize> = data
        .touch_events
        .iter()
        .filter(|t| t.player.as_ref().is_some_and(is_me))
        .map(|t| t.frame)
        .collect();
    println!("{}", serde_json::json!({ "boost": boost, "touches": touches }));
    Ok(())
}
