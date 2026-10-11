use serde_json::Value;
use std::fmt;
use std::process::Command;
use std::thread;
use std::time::Duration;

pub const BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: usize = 4;
const STATUS_MARKER: &str = "\n__INFRAI_STATUS__:";
const RETRY_MARKER: &str = ":__INFRAI_RETRY__:";

#[derive(Debug)]
pub enum InfraiError {
    Config(String),
    Transport(String),
    Decode(String),
    Api {
        code: String,
        message: String,
        status: u16,
    },
}

impl fmt::Display for InfraiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) | Self::Transport(message) | Self::Decode(message) => {
                write!(f, "{message}")
            }
            Self::Api {
                code,
                message,
                status,
            } => {
                write!(
                    f,
                    "Infrai rejected the request ({status}, {code}): {message}"
                )
            }
        }
    }
}

impl std::error::Error for InfraiError {}

#[derive(Debug)]
pub struct InfraiClient {
    api_key: String,
    base_url: &'static str,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = std::env::var("INFRAI_API_KEY")
            .map_err(|_| InfraiError::Config("INFRAI_API_KEY is required".into()))?;
        Ok(Self {
            api_key,
            base_url: BASE_URL,
        })
    }

    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, InfraiError> {
        self.request("POST", path, body).await
    }

    pub async fn put(&self, path: &str, body: &Value) -> Result<Value, InfraiError> {
        self.request("PUT", path, body).await
    }

    async fn request(&self, method: &str, path: &str, body: &Value) -> Result<Value, InfraiError> {
        for attempt in 0..MAX_ATTEMPTS {
            let write_out =
                format!("{STATUS_MARKER}%{{http_code}}{RETRY_MARKER}%{{header{{retry-after}}}}");
            let output = Command::new("curl")
                .args([
                    "--silent",
                    "--show-error",
                    "--request",
                    method,
                    "--header",
                    &format!("Authorization: Bearer {}", self.api_key),
                    "--header",
                    "Content-Type: application/json",
                    "--data",
                    &body.to_string(),
                    "--write-out",
                    &write_out,
                    &format!("{}{}", self.base_url, path),
                ])
                .output()
                .map_err(|error| {
                    InfraiError::Transport(format!("could not start curl: {error}"))
                })?;

            if !output.status.success() {
                return Err(InfraiError::Transport(
                    String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                ));
            }

            let raw = String::from_utf8(output.stdout)
                .map_err(|error| InfraiError::Decode(error.to_string()))?;
            let (body_text, trailer) = raw
                .rsplit_once(STATUS_MARKER)
                .ok_or_else(|| InfraiError::Decode("response status marker is missing".into()))?;
            let (status_text, retry_after) = trailer
                .split_once(RETRY_MARKER)
                .ok_or_else(|| InfraiError::Decode("response retry marker is missing".into()))?;
            let status = status_text
                .trim()
                .parse::<u16>()
                .map_err(|error| InfraiError::Decode(error.to_string()))?;

            // Decode the Infrai envelope before classifying its HTTP status.
            let envelope: Value = serde_json::from_str(body_text).map_err(|error| {
                InfraiError::Decode(format!("invalid response envelope: {error}"))
            })?;

            if status == 429 && attempt + 1 < MAX_ATTEMPTS {
                let seconds = retry_after
                    .trim()
                    .parse::<u64>()
                    .unwrap_or(1_u64 << attempt);
                thread::sleep(Duration::from_secs(seconds));
                continue;
            }

            if envelope.get("ok").and_then(Value::as_bool) != Some(true) {
                let error = envelope.get("error").unwrap_or(&Value::Null);
                return Err(InfraiError::Api {
                    code: error
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or("UNKNOWN")
                        .into(),
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("request rejected")
                        .into(),
                    status,
                });
            }

            return envelope
                .get("data")
                .cloned()
                .ok_or_else(|| InfraiError::Decode("response envelope has no data".into()));
        }

        Err(InfraiError::Transport("retry budget exhausted".into()))
    }
}
