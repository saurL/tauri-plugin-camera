import { invoke } from '@tauri-apps/api/core'

// Types mirrored from Rust (src/webrtc.rs)
export interface IceServer {
  urls: string[]
  username?: string
  credential?: string
}

export interface SessionDescription {
  type: 'offer' | 'answer'
  sdp: string
}

export interface IceCandidateInitLike {
  candidate: string
  sdpMid?: string
  sdpMlineIndex?: number
}

// Normalize helpers (JS camelCase -> Rust snake_case where needed)
function toRustIceCandidate(c: IceCandidateInitLike) {
  return {
    candidate: c.candidate,
    sdp_mid: c.sdpMid,
    sdp_m_line_index: c.sdpMlineIndex,
  }
}

// Create a new PeerConnection on the backend and get an SDP offer + connectionId
export async function createOffer(iceServers: IceServer[] = []): Promise<{ offer: SessionDescription; connectionId: string }> {
  const [sdpData, connectionId] = await invoke<[SessionDescription, string]>('plugin:camera|create_offer', {
    request: { ice_servers: iceServers },
  })
  // Rust returns with key `type`; align to our TS interface
  const offer: SessionDescription = { type: sdpData.type as 'offer', sdp: sdpData.sdp }
  return { offer, connectionId }
}

// Composite: initialize camera, create connection, attach track, start streaming, and return offer + connectionId
export async function startCameraWebRTCSesion(deviceId: string, iceServers: IceServer[] = []): Promise<{ offer: SessionDescription; connectionId: string }> {
  const [sdpData, connectionId] = await invoke<[SessionDescription, string]>('plugin:camera|start_camera_webrtc_session', {
    deviceId,
    iceServers: iceServers,
  })
  const offer: SessionDescription = { type: sdpData.type as 'offer', sdp: sdpData.sdp }
  return { offer, connectionId }
}

export async function setRemoteDescription(connectionId: string, description: SessionDescription): Promise<void> {
  await invoke('plugin:camera|set_remote_description', {
    connectionId,
    description: { type: description.type, sdp: description.sdp },
  })
}

export async function createAnswer(connectionId: string): Promise<SessionDescription> {
  const sdpData = await invoke<SessionDescription>('plugin:camera|create_answer', { connectionId })
  return { type: sdpData.type as 'answer', sdp: sdpData.sdp }
}

export async function addIceCandidate(connectionId: string, candidate: IceCandidateInitLike): Promise<void> {
  await invoke('plugin:camera|add_ice_candidate', {
    connectionId,
    candidate: toRustIceCandidate(candidate),
  })
}

export async function closeConnection(connectionId: string): Promise<void> {
  await invoke('plugin:camera|close_connection', { connectionId })
}

export async function getConnectionState(connectionId: string): Promise<string> {
  return invoke<string>('plugin:camera|get_connection_state', { connectionId })
}
