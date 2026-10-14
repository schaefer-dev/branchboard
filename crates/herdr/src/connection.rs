//! One connection to the herdr socket: requests and responses as JSON lines.

use std::time::Duration;

use interprocess::local_socket::tokio::prelude::*;
use interprocess::local_socket::tokio::{RecvHalf, SendHalf};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

use crate::{Endpoint, HerdrError};

/// Upper bound for a single request, including connecting.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

/// Lines longer than this are treated as a protocol error.
const MAX_LINE: usize = 16 * 1024 * 1024;

/// A line the server sent.
#[derive(Debug)]
pub(crate) enum Message {
    /// The answer to a request.
    Response {
        id: String,
        outcome: Result<Value, HerdrError>,
    },
    /// A pushed event on a subscription.
    Event { kind: String },
}

pub(crate) struct Connection {
    reader: BufReader<RecvHalf>,
    writer: SendHalf,
    next_id: u64,
    line: String,
}

impl Connection {
    pub(crate) async fn open(endpoint: &Endpoint) -> Result<Self, HerdrError> {
        let stream = tokio::time::timeout(REQUEST_TIMEOUT, endpoint.connect())
            .await
            .map_err(|_| HerdrError::Timeout)?
            .map_err(HerdrError::connect)?;
        let (reader, writer) = stream.split();
        Ok(Self {
            reader: BufReader::new(reader),
            writer,
            next_id: 1,
            line: String::new(),
        })
    }

    /// Sends a request and returns its id.
    pub(crate) async fn send(&mut self, method: &str, params: Value) -> Result<String, HerdrError> {
        let id = format!("branchboard-{}", self.next_id);
        self.next_id += 1;
        let mut line = json!({ "id": id, "method": method, "params": params }).to_string();
        line.push('\n');
        self.writer
            .write_all(line.as_bytes())
            .await
            .map_err(HerdrError::Io)?;
        Ok(id)
    }

    /// The next line from the server; `None` once it closed the connection.
    pub(crate) async fn receive(&mut self) -> Result<Option<Message>, HerdrError> {
        self.line.clear();
        let read = (&mut self.reader)
            .take(MAX_LINE as u64)
            .read_line(&mut self.line)
            .await
            .map_err(HerdrError::Io)?;
        if read == 0 {
            return Ok(None);
        }
        if !self.line.ends_with('\n') && read >= MAX_LINE {
            return Err(HerdrError::Protocol("message too long".into()));
        }
        parse_message(&self.line).map(Some)
    }

    /// Sends a request and waits for its response, skipping pushed events.
    pub(crate) async fn request<T: DeserializeOwned>(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<T, HerdrError> {
        let id = self.send(method, params).await?;
        let exchange = async {
            loop {
                match self.receive().await? {
                    Some(Message::Response {
                        id: answered,
                        outcome,
                    }) if answered == id => {
                        return outcome;
                    }
                    Some(_) => {}
                    None => return Err(HerdrError::Protocol("connection closed".into())),
                }
            }
        };
        let value = tokio::time::timeout(REQUEST_TIMEOUT, exchange)
            .await
            .map_err(|_| HerdrError::Timeout)??;
        serde_json::from_value(value)
            .map_err(|error| HerdrError::Protocol(format!("{method}: {error}")))
    }
}

/// Sends one request on a connection of its own.
pub(crate) async fn request<T: DeserializeOwned>(
    endpoint: &Endpoint,
    method: &str,
    params: Value,
) -> Result<T, HerdrError> {
    Connection::open(endpoint)
        .await?
        .request(method, params)
        .await
}

fn parse_message(line: &str) -> Result<Message, HerdrError> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| HerdrError::Protocol(format!("invalid JSON: {error}")))?;
    if let Some(kind) = value.get("event").and_then(Value::as_str) {
        return Ok(Message::Event {
            kind: kind.to_owned(),
        });
    }
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let outcome = if let Some(error) = value.get("error") {
        let text = |key: &str| {
            error
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        Err(HerdrError::Api {
            code: text("code"),
            message: text("message"),
        })
    } else if let Some(result) = value.get("result") {
        Ok(result.clone())
    } else {
        Err(HerdrError::Protocol("neither result nor error".into()))
    };
    Ok(Message::Response { id, outcome })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_results_errors_and_events() {
        let Message::Response { id, outcome } =
            parse_message(r#"{"id":"a","result":{"type":"pong"}}"#).unwrap()
        else {
            panic!("expected a response");
        };
        assert_eq!(id, "a");
        assert_eq!(outcome.unwrap()["type"], "pong");

        let Message::Response { outcome, .. } =
            parse_message(r#"{"id":"b","error":{"code":"not_found","message":"pane not found"}}"#)
                .unwrap()
        else {
            panic!("expected a response");
        };
        assert!(matches!(
            outcome,
            Err(HerdrError::Api { ref code, .. }) if code == "not_found"
        ));

        let Message::Event { kind } =
            parse_message(r#"{"event":"pane.created","data":{"pane_id":"w1:p2"}}"#).unwrap()
        else {
            panic!("expected an event");
        };
        assert_eq!(kind, "pane.created");
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            parse_message("not json"),
            Err(HerdrError::Protocol(_))
        ));
    }
}
