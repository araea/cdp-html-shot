use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::transport::Transport;
use crate::transport_actor::{TargetMessage, TransportResponse};

pub(crate) static GLOBAL_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn next_id() -> usize {
    GLOBAL_ID_COUNTER.fetch_add(1, Ordering::SeqCst) + 1
}

pub(crate) fn serde_msg(msg: &TargetMessage) -> Value {
    let message: Value =
        serde_json::from_str(msg.params["message"].as_str().unwrap().trim_matches('"')).unwrap();
    message
}

pub(crate) async fn send_and_get_msg(
    transport: Arc<Transport>,
    msg_id: usize,
    session_id: &str,
    msg: String,
) -> Result<TargetMessage> {
    let (_, target_msg) = futures::try_join!(
        transport.send(json!({
            "id": next_id(),
            "method": "Target.sendMessageToTarget",
            "params": {
                "sessionId": session_id,
                "message": msg
            }
        })),
        transport.get_target_msg(msg_id),
    )?;

    match target_msg {
        TransportResponse::Target(res) => Ok(res),
        other => Err(anyhow!("Unexpected transport response: {:?}", other)),
    }
}
