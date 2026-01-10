use tauri::{AppHandle, command, Runtime};

use crate::models::*;
use crate::Result;
use crate::CameraExt;

#[command]
pub(crate) async fn ping<R: Runtime>(
    app: AppHandle<R>,
    payload: PingRequest,
) -> Result<PingResponse> {
    Ok(PingResponse {
        value: payload.value,
    })
}

#[command]
pub(crate) async fn request_camera_permission<R: Runtime>(
    app: AppHandle<R>,
) -> Result<bool> {
    app.camera().request_permission().await
}

#[command]
pub(crate) async fn get_available_cameras<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<crabcamera::CameraDeviceInfo>> {
    app.camera().get_available_cameras().await
}
