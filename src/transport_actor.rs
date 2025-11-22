use anyhow::{Result, anyhow};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, tungstenite::Message};

use crate::general_utils;
use crate::general_utils::next_id;
use crate::transport::{Response, ShutdownSignal};

#[derive(Debug)]
pub(crate) enum TransportMessage {
    Request(Value, oneshot::Sender<Result<TransportResponse>>),
    ListenTargetMessage(u64, oneshot::Sender<Result<TransportResponse>>),
}

#[derive(Debug)]
pub(crate) enum TransportResponse {
    Response(Response),
    Target(TargetMessage),
}

pub(crate) struct TransportActor {
    pub(crate) pending_requests: HashMap<u64, oneshot::Sender<Result<TransportResponse>>>,
    pub(crate) ws_sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    pub(crate) command_rx: mpsc::Receiver<TransportMessage>,
    pub(crate) shutdown_rx: oneshot::Receiver<()>,
    pub(crate) shutdown_signal: Arc<ShutdownSignal>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct TargetMessage {
    method: String,
    pub(crate) params: Value,
}

impl TransportActor {
    pub(crate) async fn run(
        mut self,
        mut ws_stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) {
        loop {
            tokio::select! {
                Some(msg) = ws_stream.next() => {
                    match msg {
                        Ok(Message::Text(text)) => {
                            if let Ok(response) = serde_json::from_str::<Response>(&text) {
                                self.handle_res(response).await;
                            }
                            if let Ok(target_msg) = serde_json::from_str::<TargetMessage>(&text) {
                                self.handle_target_msg(target_msg).await;
                            }
                        }
                        Err(e) => {
                            self.handle_error(anyhow!("{e}")).await;
                            break;
                        }
                        _ => {}
                    }
                }

                Some(msg) = self.command_rx.recv() => {
                    match msg {
                        TransportMessage::Request(cmd, response_tx) => self.handle_req(cmd, response_tx).await,
                        TransportMessage::ListenTargetMessage(msg_id, response_tx) => self.listen_target_msg(msg_id, response_tx).await,
                    };
                }

                _ = &mut self.shutdown_rx => {
                    let command = json!({
                            "id": next_id(),
                            "method": "Browser.close",
                            "params": {}
                        });

                    let msg = Message::Text(serde_json::to_string(&command).unwrap());

                    let  _ = self.ws_sink
                        .send(msg)
                        .await
                        .is_ok();

                    let  _ = self.ws_sink
                        .close()
                        .await
                        .is_ok();

                    self.shutdown_signal.signal_shutdown();

                    break
                }

                else => break
            }
        }

        self.cleanup().await;
    }

    async fn handle_req(
        &mut self,
        command: Value,
        response_tx: oneshot::Sender<Result<TransportResponse>>,
    ) {
        let message = Message::Text(serde_json::to_string(&command).unwrap());

        match self.ws_sink.send(message).await {
            Ok(_) => {
                self.pending_requests
                    .insert(command["id"].as_u64().unwrap(), response_tx);
            }
            Err(e) => {
                let _ = response_tx.send(Err(anyhow!("Connection error: {}", e)));
            }
        }
    }

    async fn handle_res(&mut self, response: Response) {
        if let Some(sender) = self.pending_requests.remove(&response.id) {
            let _ = sender.send(Ok(TransportResponse::Response(response)));
        }
    }

    async fn handle_target_msg(&mut self, msg: TargetMessage) {
        if &msg.method != "Target.receivedMessageFromTarget" {
            return;
        }
        let message = general_utils::serde_msg(&msg);
        if message.get("id").is_none() {
            return;
        }
        if let Some(sender) = self
            .pending_requests
            .remove(&message.get("id").unwrap().as_u64().unwrap())
        {
            let _ = sender.send(Ok(TransportResponse::Target(msg)));
        }
    }

    async fn handle_error(&mut self, error: anyhow::Error) {
        for (_, sender) in self.pending_requests.drain() {
            let _ = sender.send(Err(anyhow!("Connection error: {}", error)));
        }
    }

    async fn cleanup(&mut self) {
        for (_, sender) in self.pending_requests.drain() {
            let _ = sender.send(Err(anyhow!("Connection closed")));
        }
    }

    async fn listen_target_msg(
        &mut self,
        msg_id: u64,
        response_tx: oneshot::Sender<Result<TransportResponse>>,
    ) {
        self.pending_requests.insert(msg_id, response_tx);
    }
}
