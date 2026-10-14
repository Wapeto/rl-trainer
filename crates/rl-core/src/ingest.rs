//! Connects to the Stats API socket, reconnecting every 2 s while the game is not running.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;

use crate::events::{decode, TimedEvent};
use crate::framing::JsonFramer;

pub const DEFAULT_ADDR: &str = "127.0.0.1:49123";
pub const RECONNECT_DELAY: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, PartialEq)]
pub enum IngestMsg {
    Connected { at_ms: i64 },
    Disconnected { at_ms: i64 },
    Event(TimedEvent),
    DecodeFailed { at_ms: i64, error: String },
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Runs until `shutdown` becomes true (or its sender is dropped) or `tx` is closed.
pub async fn run(addr: String, tx: mpsc::Sender<IngestMsg>, mut shutdown: watch::Receiver<bool>) {
    loop {
        if *shutdown.borrow() {
            return;
        }
        // Windows can take ~2 s to report "connection refused", so shutdown must interrupt the connect.
        let connected = tokio::select! {
            result = TcpStream::connect(&addr) => result.ok(),
            _ = shutdown.changed() => return,
        };
        if let Some(stream) = connected {
            if tx
                .send(IngestMsg::Connected { at_ms: now_ms() })
                .await
                .is_err()
            {
                return;
            }
            let keep_going = read_stream(stream, &tx, &mut shutdown).await;
            let delivered = tx
                .send(IngestMsg::Disconnected { at_ms: now_ms() })
                .await
                .is_ok();
            if !keep_going || !delivered {
                return;
            }
        }
        tokio::select! {
            () = sleep(RECONNECT_DELAY) => {}
            _ = shutdown.changed() => return,
        }
    }
}

/// Reads until the socket closes. Returns false when ingestion should stop entirely.
async fn read_stream(
    mut stream: TcpStream,
    tx: &mpsc::Sender<IngestMsg>,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    let mut framer = JsonFramer::default();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        let n = tokio::select! {
            read = stream.read(&mut buf) => match read {
                Ok(0) | Err(_) => return true,
                Ok(n) => n,
            },
            _ = shutdown.changed() => return false,
        };
        let objects = framer.push(&buf[..n]);
        let resets = framer.take_resets();
        if resets > 0 {
            let msg = IngestMsg::DecodeFailed {
                at_ms: now_ms(),
                error: format!("stream resynchronised {resets} time(s); buffered data dropped"),
            };
            if tx.send(msg).await.is_err() {
                return false;
            }
        }
        for text in objects {
            let at_ms = now_ms();
            let msg = match decode(&text) {
                Ok(event) => IngestMsg::Event(TimedEvent { at_ms, event }),
                Err(err) => IngestMsg::DecodeFailed {
                    at_ms,
                    error: err.to_string(),
                },
            };
            if tx.send(msg).await.is_err() {
                return false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::GameEvent;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    use tokio::time::timeout;

    const WAIT: Duration = Duration::from_secs(5);

    #[tokio::test]
    async fn streams_events_then_reports_disconnect() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let payload: &[u8] = br#"{"Event":"RoundStarted","Data":{"MatchGuid":""}}{"Event":"MatchEnded","Data":"{\"WinnerTeamNum\":1}"}{oops}"#;
            socket.write_all(&payload[..20]).await.unwrap();
            socket.write_all(&payload[20..]).await.unwrap();
        });
        let (tx, mut rx) = mpsc::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(run(addr, tx, stop_rx));

        let mut got = Vec::new();
        for _ in 0..5 {
            got.push(timeout(WAIT, rx.recv()).await.unwrap().unwrap());
        }
        assert!(matches!(got[0], IngestMsg::Connected { .. }));
        assert!(matches!(
            &got[1],
            IngestMsg::Event(TimedEvent {
                event: GameEvent::RoundStarted,
                ..
            })
        ));
        assert!(matches!(
            &got[2],
            IngestMsg::Event(TimedEvent {
                event: GameEvent::MatchEnded {
                    winner_team_num: Some(1)
                },
                ..
            })
        ));
        assert!(matches!(got[3], IngestMsg::DecodeFailed { .. }));
        assert!(matches!(got[4], IngestMsg::Disconnected { .. }));

        stop_tx.send(true).unwrap();
        timeout(WAIT, task).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn reports_data_dropped_by_a_framer_reset() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            socket.write_all(b"{\"truncated\":").await.unwrap();
            socket
                .write_all(&vec![b'1'; crate::framing::MAX_BUFFERED + 1])
                .await
                .unwrap();
            socket
                .write_all(br#"{"Event":"RoundStarted","Data":{}}"#)
                .await
                .unwrap();
        });
        let (tx, mut rx) = mpsc::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(run(addr, tx, stop_rx));
        let mut got = Vec::new();
        for _ in 0..4 {
            got.push(timeout(WAIT, rx.recv()).await.unwrap().unwrap());
        }
        assert!(matches!(got[0], IngestMsg::Connected { .. }));
        assert!(
            matches!(&got[1], IngestMsg::DecodeFailed { error, .. } if error.contains("dropped"))
        );
        assert!(matches!(
            &got[2],
            IngestMsg::Event(TimedEvent {
                event: GameEvent::RoundStarted,
                ..
            })
        ));
        assert!(matches!(got[3], IngestMsg::Disconnected { .. }));
        stop_tx.send(true).unwrap();
        timeout(WAIT, task).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn stops_promptly_while_waiting_to_reconnect() {
        let addr = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().to_string()
        }; // listener dropped: nothing accepts on this port
        let (tx, mut rx) = mpsc::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(run(addr, tx, stop_rx));
        tokio::time::sleep(Duration::from_millis(100)).await;
        stop_tx.send(true).unwrap();
        timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert!(rx.try_recv().is_err(), "no connection was ever made");
    }
}
