use crate::error::{Error, Result};
use crate::models::FrameEvent;
use crate::utils::yuv_to_h264;
use crabcamera::init::initialize_camera_system;
use crabcamera::permissions::PermissionInfo;
use crabcamera::{get_available_cameras, request_camera_permission};
use crabcamera::{get_recommended_format, set_callback, start_camera_preview, CameraDeviceInfo};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use tauri::{plugin::PluginApi, AppHandle, Runtime};
use tokio::sync::watch;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::Instant;
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> Result<Camera<R>> {
    let webrtc_manager = crate::webrtc::WebRTCManager::new();

    Ok(Camera {
        _app: app.clone(),
        webrtc_manager,
        active_streams: AsyncMutex::new(HashMap::new()),
    })
}

struct ActiveStream {
    camera_id: String,
    start_time: Instant,
    rx: watch::Receiver<Option<FrameEvent>>,
}
/// Access to the camera APIs.
pub struct Camera<R: Runtime> {
    _app: AppHandle<R>,
    pub webrtc_manager: crate::webrtc::WebRTCManager,
    active_streams: AsyncMutex<HashMap<String, ActiveStream>>,
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

    pub async fn start_streaming(&self, device_id: String) -> Result<String> {
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
        let _camera = start_camera_preview(device_id.clone(), Some(format))
            .await
            .map_err(|e| Error::CameraError(format!("Failed to start camera preview: {}", e)))?;

        // Create watch channel for frame events
        let (tx, rx) = watch::channel(None);

        let tx_clone = tx.clone();
        let callback = move |frame: crabcamera::CameraFrame| {
            let event = FrameEvent {
                width: frame.width,
                height: frame.height,
                data: frame.data,
                format: frame.format,
            };

            if let Err(e) = tx_clone.send(Some(event)) {
                log::error!("Failed to send frame event: {}", e);
            }
        };
        set_callback(device_id.clone(), callback)
            .await
            .map_err(|e| Error::CameraError(format!("Failed to set callback: {}", e)))?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let active_stream = ActiveStream {
            camera_id: device_id.clone(),
            start_time: Instant::now(),
            rx,
        };

        self.active_streams
            .lock()
            .await
            .insert(session_id.clone(), active_stream);

        Ok(session_id)
    }

    pub async fn stop_streaming(&self, stream_id: String) -> Result<()> {
        log::info!(" Stopping stream with stream_id: {}", stream_id);

        // First, signal the callback to stop processing frames
        let stream = self
            .active_streams
            .lock()
            .await
            .remove(&stream_id)
            .ok_or_else(|| Error::StreamNotFound(stream_id.clone()))?;

        log::info!(
            " Stream stopped for camera: {} (ran for {:?})",
            stream.camera_id.clone(),
            stream.start_time.elapsed()
        );

        // First, clear the callback to stop receiving frames
        log::info!(" Clearing callback for camera: {}", stream.camera_id);
        set_callback(stream.camera_id.clone(), |_| {})
            .await
            .map_err(|e| Error::CameraError(format!("Failed to clear callback: {}", e)))?;

        // Then stop the camera preview
        log::info!(" Stopping camera preview for device: {}", stream.camera_id);
        crabcamera::commands::capture::stop_camera_preview(stream.camera_id.clone())
            .await
            .map_err(|e| Error::CameraError(format!("Failed to stop camera: {}", e)))?;

        // WORKAROUND: Give more time for camera to fully release
        // TODO: This should be fixed in crabcamera by properly closing/dropping the camera
        log::warn!(" Waiting 500ms for camera to fully release (crabcamera limitation)");
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        crabcamera::commands::capture::release_camera(stream.camera_id.clone())
            .await
            .map_err(|e| Error::CameraError(format!("Failed to release camera: {}", e)))?;
        // When stream is dropped here, the threadpool will be dropped too
        log::info!(
            " Stream resources cleaned up for camera: {}",
            stream.camera_id
        );

        Ok(())
    }

    /// Get a copy of the receiver for a specific device_id
    /// Returns a watch receiver for consuming frame events from this device
    pub async fn get_receiver_by_device_id(
        &self,
        device_id: &str,
    ) -> Result<watch::Receiver<Option<FrameEvent>>> {
        let streams = self.active_streams.lock().await;

        for active_stream in streams.values() {
            if active_stream.camera_id == device_id {
                return Ok(active_stream.rx.clone());
            }
        }

        Err(Error::StreamNotFound(format!(
            "No active stream for device: {}",
            device_id
        )))
    }

    /// Connect a camera stream to a WebRTC connection
    /// This spawns a background task that:
    /// 1. Gets the receiver from the camera stream
    /// 2. Encodes frames to H.264
    /// 3. Pushes encoded frames to the WebRTC track
    pub async fn connect_camera_to_webrtc(
        &self,
        device_id: String,
        connection_id: String,
    ) -> Result<()> {
        // Ensure track is attached to the connection
        self.webrtc_manager
            .attach_receiver_to_connection(&connection_id)
            .await?;

        // Get a receiver for this device
        let mut receiver = self.get_receiver_by_device_id(&device_id).await?;

        // Clone manager for the background task
        let webrtc_manager = self.webrtc_manager.clone();
        let connection_id_clone = connection_id.clone();

        // Spawn background task to consume frames and push to WebRTC
        tokio::spawn(async move {
            log::info!(
                "WebRTC encoding task started for connection: {} from device: {}",
                connection_id,
                device_id
            );

            while receiver.changed().await.is_ok() {
                // Clone the current frame out of the watch ref so no borrow lives across await
                let maybe_frame = { receiver.borrow_and_update().clone() };

                match maybe_frame {
                    Some(frame) => {
                        // Accept only I420 for now; skip unsupported formats

                        match yuv_to_h264(&frame.data, frame.width, frame.height) {
                            Ok(h264) => {
                                // Assume ~30fps -> 33ms duration per frame
                                if let Err(e) = webrtc_manager
                                    .push_h264_sample(&connection_id_clone, h264, 33)
                                    .await
                                {
                                    log::error!("Failed to push H.264 sample: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to encode frame to H.264: {}", e);
                                break;
                            }
                        }
                    }
                    None => {
                        continue;
                    }
                }
            }

            log::info!(
                "WebRTC encoding task stopped for connection: {}",
                connection_id
            );
        });

        Ok(())
    }

    // Streaming methods removed to support WebRTC-based frontend streaming
}
