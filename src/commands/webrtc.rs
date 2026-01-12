use crate::error::{Error, Result};
use crate::webrtc::{CreatePeerConnectionRequest, IceCandidateData, SessionDescriptionData};
use crate::CameraExt;

use tauri::{command, AppHandle, Runtime};
use webrtc::ice_transport::ice_candidate::RTCIceCandidateInit;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

/// Create an offer and return (SDP, connection_id)
#[command]
pub async fn create_offer<R: Runtime>(
    app: AppHandle<R>,
    request: CreatePeerConnectionRequest,
) -> Result<(SessionDescriptionData, String)> {
    let manager = &app.camera().webrtc_manager;

    // Convert ice servers
    let ice_servers: Vec<RTCIceServer> = request
        .ice_servers
        .into_iter()
        .map(|server| RTCIceServer {
            urls: server.urls,
            username: server.username.unwrap_or_default(),
            credential: server.credential.unwrap_or_default(),
            ..Default::default()
        })
        .collect();

    let connection_id = manager.create_peer_connection(ice_servers).await?;
    let conn = manager.get_connection(&connection_id).await?;

    // Attach a video track before creating the offer so the SDP advertises video.
    manager.attach_h264_video_track(&connection_id).await?;

    let offer = conn
        .pc
        .create_offer(None)
        .await
        .map_err(|e| Error::CameraError(format!("Failed to create offer: {}", e)))?;

    conn.pc
        .set_local_description(offer.clone())
        .await
        .map_err(|e| Error::CameraError(format!("Failed to set local description: {}", e)))?;

    Ok((
        SessionDescriptionData {
            sdp_type: offer.sdp_type.to_string(),
            sdp: offer.sdp,
        },
        connection_id,
    ))
}

/// Create an answer
#[command]
pub async fn create_answer<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
) -> Result<SessionDescriptionData> {
    let manager = &app.camera().webrtc_manager;
    let conn = manager.get_connection(&connection_id).await?;

    let answer = conn
        .pc
        .create_answer(None)
        .await
        .map_err(|e| Error::CameraError(format!("Failed to create answer: {}", e)))?;

    conn.pc
        .set_local_description(answer.clone())
        .await
        .map_err(|e| Error::CameraError(format!("Failed to set local description: {}", e)))?;

    Ok(SessionDescriptionData {
        sdp_type: answer.sdp_type.to_string(),
        sdp: answer.sdp,
    })
}

/// Set remote description
#[command]
pub async fn set_remote_description<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
    description: SessionDescriptionData,
) -> Result<()> {
    let manager = &app.camera().webrtc_manager;
    let conn = manager.get_connection(&connection_id).await?;

    // Parse based on provided type
    let sdp = match description.sdp_type.to_lowercase().as_str() {
        "offer" => RTCSessionDescription::offer(description.sdp)
            .map_err(|e| Error::CameraError(format!("Failed to parse offer SDP: {}", e)))?,
        "answer" => RTCSessionDescription::answer(description.sdp)
            .map_err(|e| Error::CameraError(format!("Failed to parse answer SDP: {}", e)))?,
        other => {
            return Err(Error::CameraError(format!(
                "Unsupported SDP type: {}",
                other
            )))
        }
    };

    conn.pc
        .set_remote_description(sdp)
        .await
        .map_err(|e| Error::CameraError(format!("Failed to set remote description: {}", e)))?;

    Ok(())
}

/// Add ICE candidate
#[command]
pub async fn add_ice_candidate<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
    candidate: IceCandidateData,
) -> Result<()> {
    let manager = &app.camera().webrtc_manager;
    let conn = manager.get_connection(&connection_id).await?;

    let ice_candidate = RTCIceCandidateInit {
        candidate: candidate.candidate,
        sdp_mid: candidate.sdp_mid,
        sdp_mline_index: candidate.sdp_m_line_index,
        ..Default::default()
    };

    conn.pc
        .add_ice_candidate(ice_candidate)
        .await
        .map_err(|e| Error::CameraError(format!("Failed to add ICE candidate: {}", e)))?;

    Ok(())
}

/// Close peer connection
#[command]
pub async fn close_connection<R: Runtime>(app: AppHandle<R>, connection_id: String) -> Result<()> {
    let manager = &app.camera().webrtc_manager;
    manager.remove_connection(&connection_id).await
}

/// Get peer connection state
#[command]
pub async fn get_connection_state<R: Runtime>(
    app: AppHandle<R>,
    connection_id: String,
) -> Result<String> {
    let manager = &app.camera().webrtc_manager;
    let conn = manager.get_connection(&connection_id).await?;

    Ok(conn.pc.connection_state().to_string())
}

/// Composite command: initialize camera, attach track, create connection, and return offer
#[command]
pub async fn start_camera_webrtc_session<R: Runtime>(
    app: AppHandle<R>,
    device_id: String,
    ice_servers: Vec<RTCIceServer>,
) -> Result<(SessionDescriptionData, String)> {
    let camera = app.camera();
    // Initialize camera system (idempotent)
    camera.initialize().await?;

    // Create peer connection
    let ice_servers: Vec<RTCIceServer> = ice_servers
        .into_iter()
        .map(|server| RTCIceServer {
            urls: server.urls,
            username: server.username,
            credential: server.credential,
            ..Default::default()
        })
        .collect();

    let manager = &camera.webrtc_manager;
    let connection_id = manager.create_peer_connection(ice_servers).await?;

    // Register device_id for this connection (for cleanup on close)
    manager
        .register_device_for_connection(connection_id.clone(), device_id.clone())
        .await?;

    // Attach H.264 video track so SDP advertises video
    manager.attach_h264_video_track(&connection_id).await?;
    camera.start_streaming(device_id.clone()).await?;
    // Lancer le streaming caméra et lier au track WebRTC
    camera
        .connect_camera_to_webrtc(device_id, connection_id.clone())
        .await?;

    let conn = manager.get_connection(&connection_id).await?;

    let offer = conn
        .pc
        .create_offer(None)
        .await
        .map_err(|e| Error::CameraError(format!("Failed to create offer: {}", e)))?;

    conn.pc
        .set_local_description(offer.clone())
        .await
        .map_err(|e| Error::CameraError(format!("Failed to set local description: {}", e)))?;

    Ok((
        SessionDescriptionData {
            sdp_type: offer.sdp_type.to_string(),
            sdp: offer.sdp,
        },
        connection_id,
    ))
}
