//! One task per WebTransport session: decodes client datagrams into game commands and writes the
//! game's outgoing messages back, through the optional network emulator.

use crate::game::{Command, Outgoing};
use crate::netem::{DelayQueue, NetemConfig};
use crate::profile::Profile;
use bytes::Bytes;
use salvo::prelude::*;
use shared::consts::*;
use shared::protocol::*;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
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
/// Signaling messages a player may send per second: an offer, an answer and a burst of ICE candidates
/// for each of the few peers in earshot.
const MAX_RTC_PER_SECOND: u32 = 64;
/// Largest frame the signaling stream may announce: a payload and its MessagePack framing.
const MAX_RTC_FRAME: usize = RTC_MAX_PAYLOAD + 64;

/// Reads from the signaling stream if the client has opened one, and waits forever otherwise.
async fn read_signal<R: AsyncRead + Unpin>(stream: &mut Option<R>, chunk: &mut [u8]) -> std::io::Result<usize> {
    match stream {
        Some(stream) => stream.read(chunk).await,
        None => std::future::pending().await,
    }
}

/// Takes the complete `[u32 length][MessagePack]` frames off the front of `buffer`. `None` means a
/// frame announced a length no honest client sends.
fn take_frames(buffer: &mut Vec<u8>) -> Option<Vec<Vec<u8>>> {
    let mut frames = Vec::new();
    let mut at = 0;
    while buffer.len() >= at + 4 {
        let length = u32::from_be_bytes(buffer[at..at + 4].try_into().expect("four bytes")) as usize;
        if length > MAX_RTC_FRAME {
            return None;
        }
        if buffer.len() < at + 4 + length {
            break;
        }
        frames.push(buffer[at + 4..at + 4 + length].to_vec());
        at += 4 + length;
    }
    buffer.drain(..at);
    Some(frames)
}

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
        let profile = Profile::from_query(req.queries().iter_all().flat_map(|(key, values)| values.iter().map(move |value| (key.as_str(), value.as_str()))));
        let session = req.web_transport_mut().await?;

        let (outbox, mut inbox) = mpsc::channel(OUTBOX_CAPACITY);
        let (reply, response) = oneshot::channel();
        self.commands.send(Command::Join { outbox, reply, profile }).await.map_err(salvo::Error::other)?;
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
        // The one stream the client opens for voice signaling, with the bytes read so far.
        let mut signaling = None;
        let mut signal_buffer: Vec<u8> = Vec::new();
        let mut rtc_window = (Instant::now(), 0u32);
        let mut chunk = [0u8; 2048];

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
                accepted = session.accept_uni(), if signaling.is_none() => {
                    match accepted {
                        Ok(Some((_, stream))) => {
                            signaling = Some(stream);
                            signal_buffer.clear();
                        }
                        _ => break,
                    }
                }
                read = read_signal(&mut signaling, &mut chunk) => {
                    match read {
                        Ok(count) if count > 0 => {
                            last_heard = Instant::now();
                            signal_buffer.extend_from_slice(&chunk[..count]);
                            let Some(frames) = take_frames(&mut signal_buffer) else {
                                tracing::info!(id, "oversized signaling frame, dropping");
                                break;
                            };
                            for frame in frames {
                                if rtc_window.0.elapsed() >= Duration::from_secs(1) {
                                    rtc_window = (Instant::now(), 0);
                                }
                                rtc_window.1 += 1;
                                if rtc_window.1 > MAX_RTC_PER_SECOND { continue; }
                                let Some(Rtc(tag, to, kind, json)) = decode(&frame) else { continue };
                                if tag != C_RTC || kind > RTC_ICE || json.len() > RTC_MAX_PAYLOAD { continue; }
                                match self.commands.try_send(Command::Rtc { from: id, to, kind, json }) {
                                    Err(mpsc::error::TrySendError::Closed(_)) => return Ok(()),
                                    Ok(()) | Err(mpsc::error::TrySendError::Full(_)) => {}
                                }
                            }
                        }
                        // The client closed or reset it: it may open another.
                        _ => {
                            signaling = None;
                            signal_buffer.clear();
                        }
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn framed(payload: &[u8]) -> Vec<u8> {
        let mut bytes = (payload.len() as u32).to_be_bytes().to_vec();
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn frames_are_taken_whole_and_partial_ones_wait() {
        let mut buffer = framed(b"ab");
        buffer.extend(framed(b"cde"));
        let rest = framed(b"fg");
        buffer.extend_from_slice(&rest[..5]);
        assert_eq!(take_frames(&mut buffer), Some(vec![b"ab".to_vec(), b"cde".to_vec()]));
        buffer.extend_from_slice(&rest[5..]);
        assert_eq!(take_frames(&mut buffer), Some(vec![b"fg".to_vec()]));
        assert!(buffer.is_empty());
    }

    #[test]
    fn an_oversized_frame_is_refused() {
        let mut buffer = ((MAX_RTC_FRAME + 1) as u32).to_be_bytes().to_vec();
        assert_eq!(take_frames(&mut buffer), None);
    }
}
