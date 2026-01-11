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
use tauri::async_runtime::spawn;
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
use rayon::ThreadPoolBuilder;

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
        channel: Channel<crate::models::FrameEvent>,
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
        let channel_clone = channel.clone();

        // ⚡ Pool de threads pour conversions
        let pool = ThreadPoolBuilder::new()
            .num_threads(3)
            .thread_name(|i| format!("camera-convert-{}", i))
            .build()
            .unwrap();
        let pool = Arc::new(pool);
        let pool_clone = pool.clone();

        // ⚡ Compteur pour tracker les conversions actives
        let active_conversions = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let active_clone = active_conversions.clone();

        let callback = move |frame: crabcamera::CameraFrame| {
            let frame_id = counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            // Skip 2 out of 3 frames to prevent backlog
            if frame_id % 3 != 0 {
                log::debug!("⏭️  Skipping frame #{}", frame_id);
                return;
            }

            // ⚡ Vérifier si le pool est plein AVANT de spawn
            let current_active = active_clone.load(std::sync::atomic::Ordering::Relaxed);
            if current_active >= 3 {
                log::debug!(
                    "⏭️  Frame #{} skipped - pool full ({}/3 conversions active)",
                    frame_id,
                    current_active
                );
                return;
            }

            // ⚡ Incrémenter le compteur AVANT de spawn pour éviter race condition
            let new_active = active_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            // Double check au cas où plusieurs threads incrémentent simultanément
            if new_active >= 3 {
                active_clone.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                log::debug!("⏭️  Frame #{} skipped - pool became full", frame_id);
                return;
            }

            let receive_time = std::time::Instant::now();
            let frame_channel = channel_clone.clone();
            let pool_inner = pool_clone.clone();
            let active_inner = active_clone.clone();

            pool_inner.spawn(move || {
                // ⚡ Décrémenter le compteur à la fin, même en cas d'erreur
                struct DecOnDrop(Arc<std::sync::atomic::AtomicUsize>);
                impl Drop for DecOnDrop {
                    fn drop(&mut self) {
                        self.0.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    }
                }
                let _guard = DecOnDrop(active_inner);

                log::debug!("🔓 Frame #{} processing started", frame_id);

                log::info!(
                    "📹 Frame #{} received at {:?}: {}x{}, format: {}, data size: {} bytes",
                    frame_id,
                    receive_time,
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
                log::debug!(
                    "Raw data sample (first {} bytes): {}",
                    sample_size,
                    raw_sample.join(" ")
                );

                let rgb_data = match frame.format.as_str() {
                    "RGB8" => {
                        log::info!("✅ Format is already RGB8, no conversion needed");
                        frame.data
                    }
                    "YUV" => {
                        log::info!("🔄 Converting YUV to RGB8...");
                        match yuv_to_rgb(&frame.data, frame.width, frame.height) {
                            Ok(data) => {
                                log::info!(
                                    "✅ YUV conversion successful, output size: {} bytes",
                                    data.len()
                                );
                                data
                            }
                            Err(e) => {
                                log::error!("❌ YUV conversion failed: {:?}", e);
                                return; // Le guard décrémentera automatiquement
                            }
                        }
                    }
                    "NV12" => {
                        log::info!("🔄 Converting NV12 to RGB8...");
                        match nv12_to_rgb(&frame.data, frame.width, frame.height) {
                            Ok(data) => {
                                log::info!(
                                    "✅ NV12 conversion successful, output size: {} bytes",
                                    data.len()
                                );
                                data
                            }
                            Err(e) => {
                                log::error!("❌ NV12 conversion failed: {:?}", e);
                                return;
                            }
                        }
                    }
                    _ => {
                        log::error!("❌ Unsupported frame format: {}", frame.format);
                        return;
                    }
                };

                // Sample first 30 bytes of RGB data
                let rgb_sample_size = rgb_data.len().min(30);
                let rgb_sample: Vec<String> = rgb_data[..rgb_sample_size]
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect();
                log::debug!(
                    "RGB data sample (first {} bytes): {}",
                    rgb_sample_size,
                    rgb_sample.join(" ")
                );

                // Calculate expected size
                let expected_size = (frame.width * frame.height * 3) as usize;
                log::info!(
                    "📊 RGB data size: {} bytes (expected: {} bytes for {}x{} RGB8)",
                    rgb_data.len(),
                    expected_size,
                    frame.width,
                    frame.height
                );

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

                let send_time = std::time::Instant::now();
                let processing_duration = send_time.duration_since(receive_time);

                if let Err(e) = frame_channel.send(frame_event) {
                    log::error!("❌ Frame #{} failed to send: {}", frame_id, e);
                } else {
                    log::info!(
                        "✅ Frame #{} sent at {:?} (processing took {:?})",
                        frame_id,
                        send_time,
                        processing_duration
                    );
                }

                // Le guard décrémente automatiquement ici
            });
        };

        set_callback(device_id.clone(), callback)
            .await
            .map_err(|e| Error::CameraError(format!("Failed to set callback: {}", e)))?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let active_stream = ActiveStream {
            camera_id: camera,
            start_time: Instant::now(),
            frame_counter,
            channel,
        };

        self.active_streams
            .lock()
            .await
            .insert(session_id.clone(), active_stream);

        Ok(session_id)
    }

    pub async fn stop_stream(&self, session_id: String) -> Result<()> {
        log::info!("🛑 Stopping stream with session_id: {}", session_id);

        // Remove from active streams
        let stream = self
            .active_streams
            .lock()
            .await
            .remove(&session_id)
            .ok_or_else(|| Error::StreamNotFound(session_id.clone()))?;

        log::info!(
            "✅ Stream stopped for camera: {} (ran for {:?})",
            stream.camera_id,
            stream.start_time.elapsed()
        );

        // Stop the camera
        crabcamera::commands::capture::stop_camera_preview(stream.camera_id)
            .await
            .map_err(|e| Error::CameraError(format!("Failed to stop camera: {}", e)))?;

        Ok(())
    }
}
