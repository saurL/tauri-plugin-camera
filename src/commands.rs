use crate::models::*;
use crate::CameraExt;
use crate::Result;
use crabcamera::permissions::PermissionInfo;
use tauri::{command, ipc::Channel, AppHandle, Runtime};

#[command]
pub(crate) async fn request_camera_permission<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PermissionInfo> {
    app.camera().request_permission().await
}

#[command]
pub(crate) async fn get_available_cameras<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<crabcamera::CameraDeviceInfo>> {
    app.camera().get_available_cameras().await
}

#[command]
pub(crate) async fn start_streaming<R: Runtime>(
    app: AppHandle<R>,
    device_id: String,
    on_frame: Channel<FrameEvent>,
) -> Result<String> {
    app.camera().start_stream(device_id, on_frame).await
}

#[command]
pub(crate) async fn initialize<R: Runtime>(app: AppHandle<R>) -> Result<String> {
    app.camera().initialize().await
}
