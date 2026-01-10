import { invoke, Channel } from '@tauri-apps/api/core'

export interface CameraDeviceInfo {
  deviceId: string
  name: string
  description?: string
}

export interface CameraFormat {
  width: number
  height: number
  fps: number
  format?: string
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

export async function ping(value: string): Promise<string | null> {
  return await invoke<{value?: string}>('plugin:camera|ping', {
    payload: {
      value,
    },
  }).then((r) => (r.value ? r.value : null));
}

/**
 * Request camera permission from the system
 * @returns Promise with true if permission granted, false otherwise
 */
export async function requestCameraPermission(): Promise<boolean> {
  return await invoke<boolean>('plugin:camera|request_camera_permission');
}

/**
 * List all available camera devices
 */
export async function getAvailableCameras(): Promise<CameraDeviceInfo[]> {
  return await invoke<CameraDeviceInfo[]>('plugin:camera|get_available_cameras');
}

/**
 * Start streaming from a camera device
 * @param request - Stream configuration
 * @param onFrame - Callback function that receives each frame
 * @returns Promise with stream session info
 */
export async function startStreaming(
  request: StartStreamRequest,
  onFrame: (frame: FrameEvent) => void
): Promise<StartStreamResponse> {
  const channel = new Channel<FrameEvent>();
  channel.onmessage = onFrame;

  return await invoke<StartStreamResponse>('plugin:camera|start_streaming', {
    request,
    onFrame: channel,
  });
}

/**
 * Stop streaming from a camera device
 * @param deviceId - Device ID to stop streaming
 */
export async function stopStreaming(deviceId: string): Promise<void> {
  return await invoke<void>('plugin:camera|stop_streaming', {
    deviceId,
  });
}

/**
 * Capture a single frame from a camera device
 * @param deviceId - Device ID to capture from
 * @returns Promise with captured frame data
 */
export async function captureFrame(deviceId: string): Promise<FrameEvent> {
  return await invoke<FrameEvent>('plugin:camera|capture_frame', {
    deviceId,
  });
}

/**
 * Get the current format of an active camera stream
 * @param deviceId - Device ID to query
 * @returns Promise with camera format info
 */
export async function getStreamFormat(deviceId: string): Promise<CameraFormat> {
  return await invoke<CameraFormat>('plugin:camera|get_stream_format', {
    deviceId,
  });
}
