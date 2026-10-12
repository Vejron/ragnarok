//! One task per WebTransport session: decodes client datagrams into game commands and writes the
//! game's outgoing messages back, through the optional network emulator.

use crate::game::{Command, Outgoing};
use crate::netem::{DelayQueue, NetemConfig};
use bytes::Bytes;
use salvo::prelude::*;
use shared::consts::*;
use shared::protocol::*;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot};

/// Input datagrams arrive at the client tick rate; anything much faster is dropped.
const MIN_INPUT_GAP: Duration = Duration::from_millis(8);
const MIN_PING_GAP: Duration = Duration::from_millis(100);
const OUTBOX_CAPACITY: usize = 64;
/// Commands waiting for the next tick. 64 players at the input rate cap fill about 130 a tick, so
/// this is a few dozen ticks of backlog: past it the game loop is stalled and inputs are shed.
pub const COMMAND_CAPACITY: usize = 4096;
/// A session that sends nothing (not even the once-a-second ping) for this long is dropped. A tab
/// closed without a clean QUIC close would otherwise linger as a ghost until the QUIC idle timeout.
const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
/// A reliable write that has not completed after this long means the client has stopped reading.
const RELIABLE_WRITE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Transport {
    pub commands: mpsc::Sender<Command>,
    /// Production domain; `None` in development, where the Vite dev server origin is allowed.
    pub domain: Option<String>,
    pub netem: NetemConfig,
}

/// Removes the player from the game however the session ends.
struct PlayerGuard {
    commands: mpsc::Sender<Command>,
    id: u32,
}

impl Drop for PlayerGuard {
    fn drop(&mut self) {
        // A full queue must not leave a ghost player behind, so the leave waits for room.
        if let Err(mpsc::error::TrySendError::Full(command)) = self.commands.try_send(Command::Leave(self.id)) {
            let commands = self.commands.clone();
            tokio::spawn(async move {
                let _ = commands.send(command).await;
            });
        }
    }
}

impl Transport {
    fn origin_allowed(&self, origin: Option<&str>) -> bool {
        match &self.domain {
            Some(domain) => origin == Some(format!("https://{domain}").as_str()),
            None => matches!(origin, Some("http://localhost:5173" | "http://127.0.0.1:5173")),
        }
    }
}

#[handler]
impl Transport {
    async fn handle(&self, req: &mut Request) -> Result<(), salvo::Error> {
        let origin = req.headers().get("origin").and_then(|value| value.to_str().ok());
        if !self.origin_allowed(origin) {
            return Err(salvo::Error::other("Invalid origin"));
        }
        let session = req.web_transport_mut().await?;

        let (outbox, mut inbox) = mpsc::channel(OUTBOX_CAPACITY);
        let (reply, response) = oneshot::channel();
        self.commands.send(Command::Join { outbox, reply }).await.map_err(salvo::Error::other)?;
        let Ok(Some(id)) = response.await else {
            return Err(salvo::Error::other("Room full"));
        };
        let _guard = PlayerGuard { commands: self.commands.clone(), id };

        let session_id = session.session_id();
        let mut reader = session.datagram_reader();
        let mut sender = session.datagram_sender();
        let seed = crate::rng::Rng::from_time().next_u64() ^ id as u64;
        let mut delayed_in: DelayQueue<Bytes> = DelayQueue::new(self.netem, seed);
        let mut delayed_out: DelayQueue<Bytes> = DelayQueue::new(self.netem, seed.rotate_left(17));
        let netem = self.netem.enabled();
        let mut last_input = Instant::now() - Duration::from_secs(1);
        let mut last_ping = Instant::now() - Duration::from_secs(1);
        let mut last_heard = Instant::now();
        // Every reliable message goes down this one stream, in order, each behind a 4-byte length.
        // Separate streams could arrive in any order, and a kill could overtake its victim's leave.
        let mut reliable = None;

        loop {
            let wake = [delayed_in.next_due(), delayed_out.next_due()].into_iter().flatten().fold(last_heard + IDLE_TIMEOUT, Instant::min);
            // Incoming datagrams that are due this iteration (directly, or released by netem).
            let mut incoming: Vec<Bytes> = Vec::new();
            tokio::select! {
                datagram = reader.read_datagram() => {
                    let Ok(datagram) = datagram else { break };
                    let payload = datagram.into_payload();
                    last_heard = Instant::now();
                    if payload.len() > MAX_PACKET { continue; }
                    if netem { delayed_in.push(payload, Instant::now()); } else { incoming.push(payload); }
                }
                outgoing = inbox.recv() => match outgoing {
                    Some(Outgoing::Datagram(bytes)) => {
                        if netem {
                            delayed_out.push(bytes, Instant::now());
                        } else if sender.send_datagram(bytes).is_err() {
                            break;
                        }
                    }
                    Some(Outgoing::Reliable(bytes)) => {
                        if reliable.is_none() {
                            reliable = Some(session.open_uni(session_id).await?);
                        }
                        let stream = reliable.as_mut().expect("opened above");
                        let mut framed = Vec::with_capacity(4 + bytes.len());
                        framed.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
                        framed.extend_from_slice(&bytes);
                        match tokio::time::timeout(RELIABLE_WRITE_TIMEOUT, stream.write_all(&framed)).await {
                            Ok(Ok(())) => {}
                            _ => {
                                tracing::info!(id, "reliable stream stalled or closed, dropping");
                                break;
                            }
                        }
                    }
                    None => break,
                },
                _ = tokio::time::sleep_until(wake.into()) => {
                    let now = Instant::now();
                    if now >= last_heard + IDLE_TIMEOUT {
                        tracing::info!(id, "session idle, dropping");
                        break;
                    }
                    while let Some(bytes) = delayed_out.pop_due(now) {
                        if sender.send_datagram(bytes).is_err() { return Ok(()); }
                    }
                    while let Some(bytes) = delayed_in.pop_due(now) {
                        incoming.push(bytes);
                    }
                }
            }
            for payload in incoming {
                match peek_tag(&payload) {
                    Some(C_INPUT) => {
                        if last_input.elapsed() < MIN_INPUT_GAP { continue; }
                        let Some(ClientInput(_, version, baseline, frames)) = decode(&payload) else { continue };
                        if version != PROTOCOL_VERSION {
                            tracing::info!(id, version, "client speaks another protocol version, dropping");
                            return Ok(());
                        }
                        if frames.is_empty() || frames.len() > INPUT_REDUNDANCY { continue; }
                        last_input = Instant::now();
                        // A full queue means the game loop is behind: shed the input, the next one repeats it.
                        match self.commands.try_send(Command::Input(id, baseline, frames)) {
                            Err(mpsc::error::TrySendError::Closed(_)) => return Ok(()),
                            Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                        }
                    }
                    Some(C_PING) => {
                        if last_ping.elapsed() < MIN_PING_GAP { continue; }
                        let Some(Ping(_, nonce)) = decode(&payload) else { continue };
                        last_ping = Instant::now();
                        let pong = Bytes::from(encode(&Pong(S_PONG, nonce)));
                        if netem {
                            delayed_out.push(pong, Instant::now());
                        } else if sender.send_datagram(pong).is_err() {
                            return Ok(());
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
