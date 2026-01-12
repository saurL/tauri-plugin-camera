use crate::error::{Error, Result};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio::time::Duration;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

/// Video stream state
pub struct VideoStream {
    pub device_id: String,
    pub connection_id: Option<String>, // If tied to a WebRTC connection
    pub tx: mpsc::UnboundedSender<Vec<u8>>, // Send encoded H.264 data
}

/// WebRTC peer connection wrapper
pub struct PeerConnection {
    #[allow(dead_code)]
    pub id: String,
    pub pc: Arc<RTCPeerConnection>,
    pub video_track: AsyncMutex<Option<Arc<TrackLocalStaticSample>>>, // H.264 video track if attached
}

/// WebRTC manager state
#[derive(Clone)]
pub struct WebRTCManager {
    connections: Arc<AsyncMutex<HashMap<String, Arc<PeerConnection>>>>,
    streams: Arc<AsyncMutex<HashMap<String, Arc<VideoStream>>>>, // Active video streams
    connection_to_device: Arc<AsyncMutex<HashMap<String, String>>>, // Map connection_id -> device_id
}

impl WebRTCManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(AsyncMutex::new(HashMap::new())),
            streams: Arc::new(AsyncMutex::new(HashMap::new())),
            connection_to_device: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    /// Register device_id for a connection (for cleanup on close)
    pub async fn register_device_for_connection(
        &self,
        connection_id: String,
        device_id: String,
    ) -> Result<()> {
        self.connection_to_device
            .lock()
            .await
            .insert(connection_id, device_id);
        Ok(())
    }

    /// Get the device_id for a connection
    pub async fn get_device_for_connection(&self, connection_id: &str) -> Option<String> {
        self.connection_to_device
            .lock()
            .await
            .get(connection_id)
            .cloned()
    }

    /// Create a new peer connection
    pub async fn create_peer_connection(&self, ice_servers: Vec<RTCIceServer>) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        // Create a MediaEngine with default codecs
        let mut media_engine = MediaEngine::default();

        // Register default codecs for video (VP8, VP9, H264) and audio (Opus)
        media_engine
            .register_default_codecs()
            .map_err(|e| Error::CameraError(format!("Failed to register codecs: {}", e)))?;

        // Create an InterceptorRegistry with default interceptors
        let registry = register_default_interceptors(
            webrtc::interceptor::registry::Registry::new(),
            &mut media_engine,
        )
        .map_err(|e| Error::CameraError(format!("Failed to register interceptors: {}", e)))?;

        // Create the API with MediaEngine and InterceptorRegistry
        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        // Configure the peer connection with ICE servers
        let config = RTCConfiguration {
            ice_servers,
            ..Default::default()
        };

        // Create the RTCPeerConnection
        let pc =
            Arc::new(api.new_peer_connection(config).await.map_err(|e| {
                Error::CameraError(format!("Failed to create peer connection: {}", e))
            })?);

        let peer_conn = Arc::new(PeerConnection {
            id: id.clone(),
            pc: pc.clone(),
            video_track: AsyncMutex::new(None),
        });

        // Store the connection
        self.connections.lock().await.insert(id.clone(), peer_conn);

        Ok(id)
    }

    /// Get a peer connection by ID
    pub async fn get_connection(&self, id: &str) -> Result<Arc<PeerConnection>> {
        self.connections
            .lock()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| Error::CameraError(format!("Peer connection not found: {}", id)))
    }

    /// Remove a peer connection
    pub async fn remove_connection(&self, id: &str) -> Result<()> {
        let device_id = self.get_device_for_connection(id).await;

        if let Some(conn) = self.connections.lock().await.remove(id) {
            conn.pc.close().await.map_err(|e| {
                Error::CameraError(format!("Failed to close peer connection: {}", e))
            })?;
        }

        self.connection_to_device.lock().await.remove(id);

        if let Some(dev_id) = device_id {
            log::info!(
                "Closed connection {}, associated device {} (caller should clean up streaming)",
                id,
                dev_id
            );
        }
        Ok(())
    }

    /// Attach an H.264 video track to the PeerConnection.
    /// This prepares the connection to accept encoded H.264 samples.
    pub async fn attach_h264_video_track(&self, id: &str) -> Result<()> {
        let conn = self.get_connection(id).await?;
        let mut video_track_guard = conn.video_track.lock().await;

        // If already attached, do nothing
        if video_track_guard.is_some() {
            return Ok(());
        }

        // Create a static sample track for H.264 video
        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: "video/h264".to_string(),
                ..Default::default()
            },
            "tauri-camera".to_string(),
            "tauri-camera-stream".to_string(),
        ));

        // Add to PeerConnection
        conn.pc
            .add_track(Arc::clone(&track) as Arc<dyn TrackLocal + Send + Sync>)
            .await
            .map_err(|e| Error::CameraError(format!("Failed to add video track: {}", e)))?;

        *video_track_guard = Some(track);
        Ok(())
    }

    /// Push an encoded H.264 access unit to the attached video track.
    /// `data` must be an Annex B byte stream (e.g., NAL units with start codes),
    /// already encoded as H.264 matching negotiated profile/level.
    pub async fn push_h264_sample(&self, id: &str, data: Vec<u8>, duration_ms: u64) -> Result<()> {
        let conn = self.get_connection(id).await?;
        let video_track_guard = conn.video_track.lock().await;
        let track = video_track_guard
            .as_ref()
            .ok_or_else(|| Error::CameraError("No video track attached".to_string()))?;

        let sample = Sample {
            data: Bytes::from(data),
            duration: Duration::from_millis(duration_ms),
            timestamp: SystemTime::now(),
            ..Default::default()
        };

        track
            .write_sample(&sample)
            .await
            .map_err(|e| Error::CameraError(format!("Failed to write H.264 sample: {}", e)))?;

        Ok(())
    }

    /// Start a video stream from a camera device, optionally tied to a WebRTC connection
    /// Returns a session ID for managing the stream
    pub async fn start_streaming(
        &self,
        stream_id: String,
        device_id: String,
        connection_id: Option<String>,
    ) -> Result<mpsc::UnboundedReceiver<Vec<u8>>> {
        let (tx, rx) = mpsc::unbounded_channel();

        let stream = Arc::new(VideoStream {
            device_id,
            connection_id,
            tx,
        });

        self.streams.lock().await.insert(stream_id.clone(), stream);

        Ok(rx)
    }

    /// Stop a video stream and clean up resources
    pub async fn stop_streaming(&self, stream_id: &str) -> Result<()> {
        self.streams.lock().await.remove(stream_id);
        Ok(())
    }

    /// Get an active stream by ID
    pub async fn get_stream(&self, stream_id: &str) -> Result<Arc<VideoStream>> {
        self.streams
            .lock()
            .await
            .get(stream_id)
            .cloned()
            .ok_or_else(|| Error::CameraError(format!("Stream not found: {}", stream_id)))
    }

    /// Attach a receiver to a WebRTC connection for streaming
    /// This spawns a background task that consumes frames from the receiver
    /// and encodes/pushes them to the WebRTC track
    pub async fn attach_receiver_to_connection(&self, connection_id: &str) -> Result<()> {
        // Verify connection exists
        let _ = self.get_connection(connection_id).await?;

        // Ensure track is attached
        self.attach_h264_video_track(connection_id).await?;

        // NOTE: The background task that consumes frames from the receiver
        // should be spawned by the caller, as it needs access to the receiver
        // and potentially the encoder pipeline.

        Ok(())
    }
}

// ============================================================================
// Data Types for WebRTC Commands
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct IceServer {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoConfig {
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartStreamingRequest {
    pub stream_id: String,
    pub device_id: String,
    #[serde(default)]
    pub connection_id: Option<String>, // Tie to a WebRTC connection (optional)
    #[serde(default)]
    pub video: Option<VideoConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreatePeerConnectionRequest {
    #[serde(default)]
    pub ice_servers: Vec<IceServer>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartPeerCameraRequest {
    pub connection_id: String,
    pub device_id: String,
    #[serde(default)]
    pub ice_servers: Vec<IceServer>,
    #[serde(default)]
    pub video: Option<VideoConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDescriptionData {
    #[serde(rename = "type")]
    pub sdp_type: String,
    pub sdp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IceCandidateData {
    pub candidate: String,
    pub sdp_mid: Option<String>,
    pub sdp_m_line_index: Option<u16>,
}
