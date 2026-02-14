use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("Hotwired backend is not running (socket not found at {0})")]
    NotConnected(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

#[derive(Debug, Serialize)]
struct SocketRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    method: String,
    params: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SocketResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    #[allow(dead_code)]
    pub error: Option<String>,
}

pub struct HotwiredClient {
    socket_path: String,
    auth_token: Option<String>,
}

impl HotwiredClient {
    pub fn new(socket_path: Option<String>) -> Self {
        let socket_path = socket_path.unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".hotwired")
                .join("hotwired.sock")
                .to_string_lossy()
                .to_string()
        });

        let auth_token = Self::read_auth_token();

        Self {
            socket_path,
            auth_token,
        }
    }

    fn read_auth_token() -> Option<String> {
        let token_path = dirs::home_dir()?.join(".hotwired").join("auth_token");
        std::fs::read_to_string(token_path)
            .ok()
            .map(|s| s.trim().to_string())
    }

    pub async fn request(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<SocketResponse, IpcError> {
        if !std::path::Path::new(&self.socket_path).exists() {
            return Err(IpcError::NotConnected(self.socket_path.clone()));
        }

        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|e| IpcError::ConnectionFailed(e.to_string()))?;

        let request = SocketRequest {
            id: None,
            method: method.to_string(),
            params,
            token: self.auth_token.clone(),
        };

        let request_json =
            serde_json::to_string(&request).map_err(|e| IpcError::RequestFailed(e.to_string()))?;

        stream
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| IpcError::RequestFailed(e.to_string()))?;
        stream
            .write_all(b"\n")
            .await
            .map_err(|e| IpcError::RequestFailed(e.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|e| IpcError::RequestFailed(e.to_string()))?;

        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| IpcError::RequestFailed(e.to_string()))?;

        serde_json::from_str(&line).map_err(|e| IpcError::InvalidResponse(e.to_string()))
    }

    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    pub async fn health_check(&self) -> Result<SocketResponse, IpcError> {
        self.request("ping", serde_json::json!({})).await
    }
}
