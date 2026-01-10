use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingRequest {
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PingResponse {
    pub value: Option<String>,
}

// Camera format configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraFormat {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub format: Option<String>,
}

// Frame event sent to frontend
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameEvent {
    pub frame_id: u64,

    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp_ms: u64,
    pub format: String,
}

// Request to start streaming
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartStreamRequest {
    pub device_id: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
}

// Response when streaming starts
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartStreamResponse {
    pub session_id: String,
    pub format: CameraFormat,
}
