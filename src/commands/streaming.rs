use crate::error::Result;
use crate::webrtc::StartStreamingRequest;
use crate::CameraExt;
use tauri::{command, AppHandle, Runtime};

/// Start a video stream from a camera device
/// The Camera handles capture, encoding to H.264, and WebRTC integration
#[command]
pub async fn start_streaming<R: Runtime>(app: AppHandle<R>, device_id: String) -> Result<String> {
    let camera = app.camera();

    camera.start_streaming(device_id).await
}

/// Stop a video stream
#[command]
pub async fn stop_streaming<R: Runtime>(app: AppHandle<R>, stream_id: String) -> Result<()> {
    let camera = app.camera();
    camera.stop_streaming(stream_id).await
}
