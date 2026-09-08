//! Bounded read-only retries. AI calls are never replayed automatically.
use reqwest::RequestBuilder;
use serde_json::Value;
use std::time::Duration;

struct Failure {
    message: String,
    retryable: bool,
    retry_after: Option<Duration>,
}
impl Failure {
    fn new(message: impl Into<String>, retryable: bool) -> Self {
        Self {
            message: message.into(),
            retryable,
            retry_after: None,
        }
    }
    fn transport(error: reqwest::Error, stage: &str) -> Self {
        let cause = if error.is_timeout() {
            "timeout"
        } else if error.is_connect() {
            "connection failed"
        } else {
            "connection interrupted"
        };
        // Never format reqwest::Error: its URL can contain the RPC API key.
        Self::new(format!("{cause} while {stage}"), true)
    }
}

async fn attempt(request: RequestBuilder) -> Result<Value, Failure> {
    let mut response = request
        .send()
        .await
        .map_err(|e| Failure::transport(e, "sending request"))?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let mut error = Failure::new(
            format!("HTTP {status}"),
            matches!(status, 408 | 429 | 500 | 502 | 503 | 504),
        );
        error.retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs);
        // An unsupported HTTP-date must not cause us to retry earlier than requested.
        if response
            .headers()
            .contains_key(reqwest::header::RETRY_AFTER)
            && error.retry_after.is_none()
        {
            error.retryable = false;
        }
        return Err(error);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| Failure::transport(e, "reading response body"))?
    {
        if bytes.len() + chunk.len() > 1_048_576 {
            return Err(Failure::new("response too large", false));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| Failure::new("invalid JSON response", false))?;
    if let Some(code) = value["error"]["code"].as_i64() {
        let detail = match code {
            -32016 => "minimum context slot not reached",
            -32005 => "node unhealthy",
            _ => "request rejected",
        };
        return Err(Failure::new(
            format!("RPC error {code}: {detail}"),
            matches!(code, -32016 | -32005),
        ));
    }
    Ok(value)
}

pub async fn json(
    request: RequestBuilder,
    service: &str,
    retry_reads: bool,
    budget: Duration,
) -> Result<Value, String> {
    let deadline = tokio::time::Instant::now() + budget;
    let attempts = if retry_reads { 3 } else { 1 };
    for number in 1..=attempts {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let timeout = if retry_reads {
            remaining.min(Duration::from_secs(3))
        } else {
            remaining
        };
        let cloned = request
            .try_clone()
            .ok_or_else(|| format!("{service}: request cannot be replayed"))?;
        let result = tokio::time::timeout(timeout, attempt(cloned.timeout(timeout)))
            .await
            .unwrap_or_else(|_| Err(Failure::new("request/response timeout", true)));
        match result {
            Ok(value) => return Ok(value),
            Err(error) => {
                let delay = error
                    .retry_after
                    .unwrap_or(Duration::from_millis(300 * number));
                if !error.retryable
                    || number == attempts
                    || delay >= deadline.saturating_duration_since(tokio::time::Instant::now())
                {
                    return Err(format!(
                        "{service}: {} ({number} attempt(s), {}s budget)",
                        error.message,
                        budget.as_secs()
                    ));
                }
                tokio::time::sleep(delay).await;
            }
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct Server {
        url: String,
        calls: Arc<AtomicUsize>,
        task: tokio::task::JoinHandle<()>,
    }
    impl Drop for Server {
        fn drop(&mut self) {
            self.task.abort();
        }
    }
    fn response(status: u16, body: &str, extra: &str) -> String {
        format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n{body}", body.len())
    }
    async fn serve(replies: Vec<(String, Duration)>) -> Server {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/?api-key=private-key",
            listener.local_addr().unwrap()
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 8192];
                if socket.read(&mut request).await.unwrap_or(0) == 0 {
                    continue;
                }
                let index = counter
                    .fetch_add(1, Ordering::SeqCst)
                    .min(replies.len() - 1);
                let (wire, delay) = &replies[index];
                let (head, body) = wire.split_once("\r\n\r\n").unwrap();
                let _ = socket.write_all(format!("{head}\r\n\r\n").as_bytes()).await;
                tokio::time::sleep(*delay).await;
                let _ = socket.write_all(body.as_bytes()).await;
            }
        });
        Server { url, calls, task }
    }
    #[tokio::test]
    async fn retries_lag_rate_limit_and_interrupted_body_with_a_finite_budget() {
        for first in [
            response(
                200,
                r#"{"error":{"code":-32016,"message":"private-key"}}"#,
                "",
            ),
            response(429, "private-key", "Retry-After: 0\r\n"),
            "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{".into(),
        ] {
            let server = serve(vec![
                (first, Duration::ZERO),
                (response(200, r#"{"result":42}"#, ""), Duration::ZERO),
            ])
            .await;
            let result = json(
                reqwest::Client::new().get(&server.url),
                "Market RPC",
                true,
                Duration::from_secs(2),
            )
            .await
            .unwrap();
            assert_eq!(result["result"], 42);
            assert_eq!(server.calls.load(Ordering::SeqCst), 2);
        }
        let server = serve(vec![(response(503, "private-key", ""), Duration::ZERO)]).await;
        let error = json(
            reqwest::Client::new().get(&server.url),
            "Market RPC",
            true,
            Duration::from_secs(2),
        )
        .await
        .unwrap_err();
        assert!(error.contains("HTTP 503 (3 attempt(s)"));
        assert!(!error.contains("private-key"));
        assert_eq!(server.calls.load(Ordering::SeqCst), 3);
    }
    #[tokio::test]
    async fn does_not_retry_auth_invalid_json_or_long_retry_after() {
        for wire in [
            response(401, "private-key", ""),
            response(200, "invalid private-key", ""),
            response(429, "", "Retry-After: 60\r\n"),
        ] {
            let server = serve(vec![(wire, Duration::ZERO)]).await;
            let started = tokio::time::Instant::now();
            let error = json(
                reqwest::Client::new().get(&server.url),
                "Jupiter",
                true,
                Duration::from_secs(2),
            )
            .await
            .unwrap_err();
            assert!(error.starts_with("Jupiter:"));
            assert!(!error.contains("private-key"));
            assert_eq!(server.calls.load(Ordering::SeqCst), 1);
            assert!(started.elapsed() < Duration::from_secs(1));
        }
    }
    #[tokio::test]
    async fn ai_body_uses_its_own_timeout_and_is_never_replayed() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(10))
            .build()
            .unwrap();
        let server = serve(vec![(
            response(200, r#"{"result":true}"#, ""),
            Duration::from_millis(60),
        )])
        .await;
        assert!(json(
            client.post(&server.url),
            "AI onchain_analyst",
            false,
            Duration::from_secs(1)
        )
        .await
        .is_ok());
        assert_eq!(server.calls.load(Ordering::SeqCst), 1);
        let server = serve(vec![(
            response(200, r#"{"result":true}"#, ""),
            Duration::from_millis(200),
        )])
        .await;
        let error = json(
            client.post(&server.url),
            "AI onchain_analyst",
            false,
            Duration::from_millis(50),
        )
        .await
        .unwrap_err();
        assert!(error.contains("AI onchain_analyst:") && error.contains("timeout"));
        assert_eq!(server.calls.load(Ordering::SeqCst), 1);
    }
}
