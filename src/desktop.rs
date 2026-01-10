use crate::error::{Error, Result};
use crate::utils::{nv12_to_rgb, yuv_to_rgb};
use crabcamera::init::initialize_camera_system;
use crabcamera::permissions::PermissionInfo;
use crabcamera::CameraDeviceInfo;
use crabcamera::{
    get_available_cameras, get_recommended_format, request_camera_permission, set_callback,
    start_camera_preview,
};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tauri::{ipc::Channel, plugin::PluginApi, AppHandle, Runtime};
use tokio::sync::Mutex as AsyncMutex;

pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<Camera<R>> {
    Ok(Camera {
        app: app.clone(),
        active_streams: Arc::new(AsyncMutex::new(HashMap::new())),
    })
}

struct ActiveStream {
    camera_id: String,
    start_time: Instant,
    frame_counter: Arc<std::sync::atomic::AtomicU64>,
    channel: Channel<crate::models::FrameEvent>,
}

/// Access to the camera APIs.
pub struct Camera<R: Runtime> {
    app: AppHandle<R>,
    active_streams: Arc<AsyncMutex<HashMap<String, ActiveStream>>>,
}

impl<R: Runtime> Camera<R> {
    /// Request camera permission from the system
    pub async fn request_permission(&self) -> Result<PermissionInfo> {
        request_camera_permission()
            .await
            .map_err(|e| Error::CameraError(format!("Failed to request camera permission: {}", e)))
    }

    pub async fn initialize(&self) -> Result<String> {
        initialize_camera_system()
            .await
            .map_err(|e| Error::CameraError(format!("Failed to initialize camera system: {}", e)))
    }

    /// List all available camera devices
    pub async fn get_available_cameras(&self) -> Result<Vec<CameraDeviceInfo>> {
        let devices = get_available_cameras()
            .await
            .map_err(|e| Error::CameraError(format!("Failed to list devices: {}", e)))?;

        Ok(devices)
    }

    pub async fn start_stream_default_camera(
        &self,
        on_frame: Channel<crate::models::FrameEvent>,
    ) -> Result<String> {
        let devices = self.get_available_cameras().await?;
        if let Some(camera) = devices.first() {
            self.start_stream(camera.id.clone(), on_frame).await
        } else {
            Err(Error::CameraError("No camera devices found".to_string()))
        }
    }

    pub async fn start_stream(
        &self,
        device_id: String,
        on_frame: Channel<crate::models::FrameEvent>,
    ) -> Result<String> {
        // Check if streaming is already active for this device
        {
            let streams = self.active_streams.lock().await;
            for active_stream in streams.values() {
                if active_stream.camera_id == device_id {
                    return Err(Error::StreamingAlreadyActive(device_id));
                }
            }
        }

        let format = get_recommended_format()
            .await
            .map_err(|e| Error::CameraError(format!("Failed to get recommended format : {}", e)))?;
        let camera = start_camera_preview(device_id.clone(), Some(format))
            .await
            .map_err(|e| Error::CameraError(format!("Failed to start camera preview: {}", e)))?;

        let frame_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let counter_clone = frame_counter.clone();
        let channel_clone = on_frame.clone();

        let callback = move |frame: crabcamera::CameraFrame| {
            // Log frame info BEFORE conversion
            log::info!(
                "📹 Received frame #{}: {}x{}, format: {}, data size: {} bytes",
                counter_clone.load(std::sync::atomic::Ordering::SeqCst),
                frame.width,
                frame.height,
                frame.format,
                frame.data.len()
            );

            // Sample first 30 bytes of raw data
            let sample_size = frame.data.len().min(30);
            let raw_sample: Vec<String> = frame.data[..sample_size]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect();
            log::debug!("Raw data sample (first {} bytes): {}", sample_size, raw_sample.join(" "));

            let rgb_data = match frame.format.as_str() {
                "RGB8" => {
                    log::info!("✅ Format is already RGB8, no conversion needed");
                    frame.data
                },
                "YUV" => {
                    log::info!("🔄 Converting YUV to RGB8...");
                    match yuv_to_rgb(&frame.data, frame.width, frame.height) {
                        Ok(data) => {
                            log::info!("✅ YUV conversion successful, output size: {} bytes", data.len());
                            data
                        },
                        Err(e) => {
                            log::error!("❌ YUV conversion failed: {:?}", e);
                            vec![]
                        }
                    }
                },
                "NV12" => {
                    log::info!("🔄 Converting NV12 to RGB8...");
                    match nv12_to_rgb(&frame.data, frame.width, frame.height) {
                        Ok(data) => {
                            log::info!("✅ NV12 conversion successful, output size: {} bytes", data.len());
                            data
                        },
                        Err(e) => {
                            log::error!("❌ NV12 conversion failed: {:?}", e);
                            vec![]
                        }
                    }
                },
                _ => {
                    log::error!("❌ Unsupported frame format: {}", frame.format);
                    return;
                }
            };

            if rgb_data.is_empty() {
                log::error!("❌ RGB data is empty after conversion, skipping frame");
                return;
            }

            // Sample first 30 bytes of RGB data
            let rgb_sample_size = rgb_data.len().min(30);
            let rgb_sample: Vec<String> = rgb_data[..rgb_sample_size]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect();
            log::debug!("RGB data sample (first {} bytes): {}", rgb_sample_size, rgb_sample.join(" "));

            // Calculate expected size
            let expected_size = (frame.width * frame.height * 3) as usize;
            log::info!(
                "📊 RGB data size: {} bytes (expected: {} bytes for {}x{} RGB8)",
                rgb_data.len(),
                expected_size,
                frame.width,
                frame.height
            );

            let frame_id = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            let frame_event = crate::models::FrameEvent {
                frame_id,
                data: rgb_data,
                width: frame.width,
                height: frame.height,
                timestamp_ms,
                format: "RGB8".to_string(),
            };

            if let Err(e) = channel_clone.send(frame_event) {
                log::error!("❌ Failed to send frame through channel: {}", e);
            } else {
                log::debug!("✅ Frame #{} sent successfully", frame_id);
            }
        };

        set_callback(device_id.clone(), callback)
            .await
            .map_err(|e| Error::CameraError(format!("Failed to set callback: {}", e)))?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let active_stream = ActiveStream {
            camera_id: camera,
            start_time: Instant::now(),
            frame_counter,
            channel: on_frame,
        };

        self.active_streams
            .lock()
            .await
            .insert(session_id.clone(), active_stream);

        Ok(session_id)
    }
}
