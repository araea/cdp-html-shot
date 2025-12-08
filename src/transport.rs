#![allow(dead_code)]

use anyhow::{Result, anyhow};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio::time;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

pub(crate) static GLOBAL_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn next_id() -> usize {
    GLOBAL_ID_COUNTER.fetch_add(1, Ordering::SeqCst) + 1
}

#[derive(Debug)]
pub(crate) enum TransportMessage {
    Request(Value, oneshot::Sender<Result<TransportResponse>>),
    ListenTargetMessage(u64, oneshot::Sender<Result<TransportResponse>>),
    WaitForEvent(String, String, oneshot::Sender<()>),
    Shutdown,
}

#[derive(Debug)]
pub(crate) enum TransportResponse {
    Response(Response),
    Target(TargetMessage),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Response {
    pub(crate) id: u64,
    pub(crate) result: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TargetMessage {
    pub(crate) params: Value,
}

struct TransportActor {
    pending_requests: HashMap<u64, oneshot::Sender<Result<TransportResponse>>>,
    event_listeners: HashMap<(String, String), Vec<oneshot::Sender<()>>>,
    ws_sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    command_rx: mpsc::Receiver<TransportMessage>,
}

impl TransportActor {
    async fn run(mut self, mut ws_stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>) {
        loop {
            tokio::select! {
                Some(msg) = ws_stream.next() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(response) = serde_json::from_str::<Response>(&text) {
                                if let Some(sender) = self.pending_requests.remove(&response.id) {
                                    let _ = sender.send(Ok(TransportResponse::Response(response)));
                                }
                            }
                            else if let Ok(target_msg) = serde_json::from_str::<TargetMessage>(&text)
                                && let Some(inner_str) = target_msg.params.get("message").and_then(|v| v.as_str())
                                    && let Ok(inner_json) = serde_json::from_str::<Value>(inner_str) {

                                        if let Some(id) = inner_json.get("id").and_then(|i| i.as_u64()) {
                                            if let Some(sender) = self.pending_requests.remove(&id) {
                                                let _ = sender.send(Ok(TransportResponse::Target(target_msg)));
                                            }
                                        }
                                        else if let Some(method) = inner_json.get("method").and_then(|s| s.as_str())
                                            && let Some(session_id) = target_msg.params.get("sessionId").and_then(|s| s.as_str()) {
                                                let key = (session_id.to_string(), method.to_string());
                                                if let Some(senders) = self.event_listeners.remove(&key) {
                                                    for tx in senders {
                                                        let _ = tx.send(());
                                                    }
                                                }
                                            }
                                    }
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }
                Some(msg) = self.command_rx.recv() => {
                    match msg {
                        TransportMessage::Request(cmd, tx) => {
                            if let Some(id) = cmd["id"].as_u64()
                                && let Ok(text) = serde_json::to_string(&cmd) {
                                    if self.ws_sink.send(Message::Text(text.into())).await.is_ok() {
                                        self.pending_requests.insert(id, tx);
                                    } else {
                                        let _ = tx.send(Err(anyhow!("WebSocket send failed")));
                                    }
                                }
                        },
                        TransportMessage::ListenTargetMessage(id, tx) => {
                            self.pending_requests.insert(id, tx);
                        },
                        TransportMessage::WaitForEvent(session_id, method, tx) => {
                            self.event_listeners.entry((session_id, method)).or_default().push(tx);
                        },
                        TransportMessage::Shutdown => {
                            let _ = self.ws_sink.send(Message::Text(json!({
                                "id": next_id(),
                                "method": "Browser.close",
                                "params": {}
                            }).to_string().into())).await;
                            let _ = self.ws_sink.close().await;
                            break;
                        }
                    }
                }
                else => break,
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Transport {
    tx: mpsc::Sender<TransportMessage>,
}

impl Transport {
    pub(crate) async fn new(ws_url: &str) -> Result<Self> {
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (ws_sink, ws_stream) = ws_stream.split();
        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let actor = TransportActor {
                pending_requests: HashMap::new(),
                event_listeners: HashMap::new(),
                ws_sink,
                command_rx: rx,
            };
            actor.run(ws_stream).await;
        });

        Ok(Self { tx })
    }

    pub(crate) async fn send(&self, command: Value) -> Result<TransportResponse> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(TransportMessage::Request(command, tx))
            .await
            .map_err(|_| anyhow!("Transport actor dropped"))?;
        time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| anyhow!("Timeout waiting for response"))?
            .map_err(|_| anyhow!("Response channel closed"))?
    }

    pub(crate) async fn get_target_msg(&self, msg_id: usize) -> Result<TransportResponse> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(TransportMessage::ListenTargetMessage(msg_id as u64, tx))
            .await
            .map_err(|_| anyhow!("Transport actor dropped"))?;
        time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| anyhow!("Timeout waiting for target message"))?
            .map_err(|_| anyhow!("Response channel closed"))?
    }

    pub(crate) async fn listen_for_event(
        &self,
        session_id: &str,
        method: &str,
    ) -> Result<oneshot::Receiver<()>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(TransportMessage::WaitForEvent(
                session_id.to_string(),
                method.to_string(),
                tx,
            ))
            .await
            .map_err(|_| anyhow!("Transport actor dropped"))?;
        Ok(rx)
    }

    pub(crate) async fn wait_for_event(&self, session_id: &str, method: &str) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(TransportMessage::WaitForEvent(
                session_id.to_string(),
                method.to_string(),
                tx,
            ))
            .await
            .map_err(|_| anyhow!("Transport actor dropped"))?;

        time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| anyhow!("Timeout waiting for event {}", method))?
            .map_err(|_| anyhow!("Event channel closed"))?;
        Ok(())
    }

    pub(crate) async fn shutdown(&self) {
        let _ = self.tx.send(TransportMessage::Shutdown).await;
    }
}
