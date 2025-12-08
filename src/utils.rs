use crate::transport::{TargetMessage, Transport, TransportResponse, next_id};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::sync::Arc;

pub(crate) fn serde_msg(msg: &TargetMessage) -> Result<Value> {
    let str_msg = msg.params["message"]
        .as_str()
        .ok_or_else(|| anyhow!("Invalid message format"))?;
    Ok(serde_json::from_str(str_msg)?)
}

pub(crate) async fn send_and_get_msg(
    transport: Arc<Transport>,
    msg_id: usize,
    session_id: &str,
    msg: String,
) -> Result<TargetMessage> {
    let send_fut = transport.send(json!({
        "id": next_id(),
        "method": "Target.sendMessageToTarget",
        "params": { "sessionId": session_id, "message": msg }
    }));
    let recv_fut = transport.get_target_msg(msg_id);

    let (_, target_msg) = futures_util::try_join!(send_fut, recv_fut)?;

    match target_msg {
        TransportResponse::Target(res) => Ok(res),
        other => Err(anyhow!("Unexpected response: {:?}", other)),
    }
}
