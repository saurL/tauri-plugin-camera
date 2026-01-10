import { invoke, Channel } from '@tauri-apps/api/core'

// Backend types -------------------------------------------------------------
export type Platform = 'Windows' | 'MacOS' | 'Linux' | 'Unknown'

export enum PermissionStatus {
  Granted = 'Granted',
  Denied = 'Denied',
  NotDetermined = 'NotDetermined',
  Restricted = 'Restricted',
}

export interface PermissionInfo {
  status: PermissionStatus
  message: string
  canRequest: boolean
}

// Raw shapes returned by crabcamera (snake_case)
interface RawPermissionInfo {
  status: PermissionStatus
  message: string
  can_request: boolean
}

interface RawCameraFormat {
  width: number
  height: number
  fps: number
  format_type: string
}

interface RawCameraDeviceInfo {
  id: string
  name: string
  description?: string
  is_available: boolean
  supports_formats: RawCameraFormat[]
  platform: Platform
}

export interface CameraFormat {
  width: number
  height: number
  fps: number
  formatType: string
}

export interface CameraDeviceInfo {
  id: string
  name: string
  description?: string
  isAvailable: boolean
  supportsFormats: CameraFormat[]
  platform: Platform
}

export interface FrameEvent {
  frameId: number
  data: Uint8Array
  width: number
  height: number
  timestampMs: number
  format: string
}

export interface StartStreamRequest {
  deviceId: string
  width?: number
  height?: number
  fps?: number
}

export interface StartStreamResponse {
  sessionId: string
  format: CameraFormat
}

const normalizePermission = (raw: RawPermissionInfo): PermissionInfo => ({
  status: raw.status,
  message: raw.message,
  canRequest: raw.can_request,
})

const normalizeFormat = (raw: RawCameraFormat): CameraFormat => ({
  width: raw.width,
  height: raw.height,
  fps: raw.fps,
  formatType: raw.format_type,
})

const normalizeDevice = (raw: RawCameraDeviceInfo): CameraDeviceInfo => ({
  id: raw.id,
  name: raw.name,
  description: raw.description,
  isAvailable: raw.is_available,
  supportsFormats: raw.supports_formats.map(normalizeFormat),
  platform: raw.platform,
})


/** Request camera permission from the system. */
export async function requestCameraPermission(): Promise<PermissionInfo> {
  const raw = await invoke<RawPermissionInfo>('plugin:camera|request_camera_permission')
  return normalizePermission(raw)
}

/** List all available camera devices (camelCased for frontend use). */
export async function getAvailableCameras(): Promise<CameraDeviceInfo[]> {
  const raw = await invoke<RawCameraDeviceInfo[]>('plugin:camera|get_available_cameras')
  return raw.map(normalizeDevice)
}

/**
 * Start streaming from a camera device.
 * The backend is expected to emit frames through the provided channel.
 */
export async function startStreaming(
  request: StartStreamRequest,
  onEvent: (frame: FrameEvent) => void
): Promise<StartStreamResponse> {
  // Use a Tauri Channel for ordered, low-latency frame delivery.
  const channel = new Channel<FrameEvent>()
  channel.onmessage = onEvent

 
  return invoke<StartStreamResponse>('plugin:camera|start_streaming', {
    request,
    onEvent: channel,
  })
}

/** Stop an active stream identified by its session id. */
export async function stopStreaming(sessionId: string): Promise<void> {
  return invoke<void>('plugin:camera|stop_streaming', { sessionId })
}

/** Capture a single frame from a running stream. */
export async function captureFrame(sessionId: string): Promise<FrameEvent> {
  return invoke<FrameEvent>('plugin:camera|capture_frame', { sessionId })
}

/** Get the current output format of a running stream. */
export async function getStreamFormat(sessionId: string): Promise<CameraFormat> {
  return invoke<CameraFormat>('plugin:camera|get_stream_format', { sessionId })
}
