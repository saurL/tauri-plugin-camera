use crate::CameraExt;
use crate::Result;
use crabcamera::permissions::PermissionInfo;
use tauri::{command, AppHandle, Runtime};

#[command]
pub async fn request_camera_permission<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PermissionInfo> {
    app.camera().request_permission().await
}

#[command]
pub async fn get_available_cameras<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Vec<crabcamera::CameraDeviceInfo>> {
    app.camera().get_available_cameras().await
}

#[command]
pub async fn initialize<R: Runtime>(app: AppHandle<R>) -> Result<String> {
    app.camera().initialize().await
}
