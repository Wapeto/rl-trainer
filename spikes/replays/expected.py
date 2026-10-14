"""Prints the replay stats rl-trainer should extract, computed independently from the probe's JSON."""
import json, sys

args = sys.argv[1:]
data = json.load(open(args[0], encoding="utf-8"))
if len(args) == 1:
    # Only the stats: print the Epic id of the replay's first human player (the tests' "me").
    meta = data["replay_meta"]
    humans = [p["remote_id"]["Epic"] for team in ("team_zero", "team_one") for p in meta[team]
              if not p["stats"].get("bBot") and "Epic" in p["remote_id"]]
    print(humans[0])
    sys.exit(0)
stats_path, boost_path, me = args
m = data["modules"]
boost = json.load(open(boost_path, encoding="utf-8"))["boost"]

def mine(mod):
    for p in m[mod].get("player_stats") or []:
        if p["player_id"].get("Epic") == me:
            return p["stats"]
    return None

def at(frame):
    frame = min(frame, len(boost) - 1)
    while frame > 0 and boost[frame] is None:
        frame -= 1
    return boost[frame] or 0.0

pos, rot, bst, wh = mine("positioning"), mine("rotation"), mine("boost"), mine("whiff")
tracked = pos["tracked_time"]
pct = lambda k: pos[k] * 100 / tracked if tracked else 0.0
picks = [e for e in m["boost"]["pickup_events"] if e["player_id"].get("Epic") == me]
touches = [at(e["frame"]) for e in m["touch"]["events"] if (e.get("player") or {}).get("Epic") == me]
fw = fl = flow = kw = kl = 0
for e in m["fifty_fifty"]["events"]:
    zero = (e.get("team_zero_player") or {}).get("Epic") == me
    one = (e.get("team_one_player") or {}).get("Epic") == me
    if not (zero or one):
        continue
    won = None if e["winning_team_is_team_0"] is None else e["winning_team_is_team_0"] == zero
    if e["is_kickoff"]:
        kw += won is True; kl += won is False
    else:
        fw += won is True; fl += won is False; flow += at(e["start_frame"]) < 20
print(json.dumps({
    "tracked_seconds": round(tracked, 3),
    "pct_behind_ball": round(pct("time_behind_ball"), 3),
    "pct_defensive_third": round(pct("time_defensive_third"), 3),
    "secs_first_man": rot and round(rot["time_first_man"], 3),
    "big_pads": bst["big_pads_collected"],
    "small_pads": sum(1 for e in picks if e["pad_type"] == "small"),
    "pads_stolen": sum(1 for e in picks if e["is_steal"]),
    "touches": len(touches),
    "touches_low_boost": sum(1 for b in touches if b < 20),
    "touches_zero_boost": sum(1 for b in touches if b < 1),
    "fifty_wins": fw, "fifty_losses": fl, "fifty_low_boost": flow,
    "kickoff_wins": kw, "kickoff_losses": kl,
    "whiffs": wh["whiff_count"],
}, indent=1))
