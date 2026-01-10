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
  const raw = await invoke<RawPermissionInfo>('plugin:camera|request-camera-permission')
  return normalizePermission(raw)
}

/** List all available camera devices (camelCased for frontend use). */
export async function getAvailableCameras(): Promise<CameraDeviceInfo[]> {
  const raw = await invoke<RawCameraDeviceInfo[]>('plugin:camera|get-available-cameras')
  return raw.map(normalizeDevice)
}

/**
 * Start streaming from a camera device.
 * The backend sends frames through the provided channel callback.
 * @param deviceId - The ID of the camera device to stream from
 * @param onFrame - Callback function that receives each frame
 * @returns Promise resolving to the session ID
 */
export async function startStreaming(
  deviceId: string,
  onFrame: (frame: FrameEvent) => void
): Promise<string> {
  // Use a Tauri Channel for ordered, low-latency frame delivery.
  const channel = new Channel<FrameEvent>()
  channel.onmessage = onFrame

  return invoke<string>('plugin:camera|start-streaming', {
    deviceId,
    onFrame: channel,
  })
}

/** Initialize the camera system. Must be called before using any camera functions. */
export async function initialize(): Promise<string> {
  return invoke<string>('plugin:camera|initialize')
}

// ============================================================================
// Utility functions for rendering frames
// ============================================================================

/**
 * Renders a camera frame to an HTML canvas element.
 * The frame data is expected to be in RGB8 format (3 bytes per pixel).
 *
 * @param canvas - The HTMLCanvasElement to render to
 * @param frame - The frame event containing image data
 * @param options - Optional rendering options
 */
export function renderFrameToCanvas(
  canvas: HTMLCanvasElement,
  frame: FrameEvent,
  options?: {
    /** Whether to scale the canvas to fit the frame size (default: true) */
    autoResize?: boolean
    /** Whether to flip the image horizontally (mirror effect, default: false) */
    flipHorizontal?: boolean
    /** Whether to flip the image vertically (default: false) */
    flipVertical?: boolean
  }
): void {
  const { autoResize = true, flipHorizontal = false, flipVertical = false } = options || {}

  // Resize canvas if needed
  if (autoResize && (canvas.width !== frame.width || canvas.height !== frame.height)) {
    canvas.width = frame.width
    canvas.height = frame.height
  }

  const ctx = canvas.getContext('2d')
  if (!ctx) {
    throw new Error('Failed to get 2D context from canvas')
  }

  // Create ImageData from the frame
  const imageData = ctx.createImageData(frame.width, frame.height)

  // Convert RGB8 to RGBA (adding alpha channel)
  const rgbData = frame.data
  const rgbaData = imageData.data

  for (let i = 0, j = 0; i < rgbData.length; i += 3, j += 4) {
    rgbaData[j] = rgbData[i]       // R
    rgbaData[j + 1] = rgbData[i + 1] // G
    rgbaData[j + 2] = rgbData[i + 2] // B
    rgbaData[j + 3] = 255            // A (fully opaque)
  }

  // Apply transformations if needed
  if (flipHorizontal || flipVertical) {
    ctx.save()
    ctx.scale(flipHorizontal ? -1 : 1, flipVertical ? -1 : 1)
    ctx.translate(
      flipHorizontal ? -frame.width : 0,
      flipVertical ? -frame.height : 0
    )
    ctx.putImageData(imageData, 0, 0)
    ctx.restore()
  } else {
    ctx.putImageData(imageData, 0, 0)
  }
}

/**
 * Creates a video element-like streaming component that automatically renders frames.
 * Returns a controller object to manage the stream.
 *
 * @param canvas - The canvas element to render to
 * @param deviceId - The camera device ID to stream from
 * @param options - Optional rendering options
 * @returns A promise that resolves to a stream controller
 *
 * @example
 * const canvas = document.getElementById('camera-preview') as HTMLCanvasElement
 * const stream = await createCameraStream(canvas, '0')
 *
 * // Later, to stop:
 * stream.stop()
 */
export async function createCameraStream(
  canvas: HTMLCanvasElement,
  deviceId: string,
  options?: {
    autoResize?: boolean
    flipHorizontal?: boolean
    flipVertical?: boolean
    /** Callback for each frame (optional) */
    onFrame?: (frame: FrameEvent) => void
    /** Callback for errors (optional) */
    onError?: (error: Error) => void
  }
): Promise<{
  sessionId: string
  stop: () => void
  canvas: HTMLCanvasElement
  /** Get the latest frame info */
  getFrameInfo: () => { frameId: number; fps: number; width: number; height: number } | null
}> {
  let lastFrameInfo: FrameEvent | null = null
  let frameCount = 0
  let startTime = Date.now()
  let running = true

  const sessionId = await startStreaming(deviceId, (frame) => {
    if (!running) return

    try {
      lastFrameInfo = frame
      frameCount++

      // Render the frame
      renderFrameToCanvas(canvas, frame, {
        autoResize: options?.autoResize,
        flipHorizontal: options?.flipHorizontal,
        flipVertical: options?.flipVertical,
      })

      // Call user callback if provided
      options?.onFrame?.(frame)
    } catch (error) {
      options?.onError?.(error as Error)
    }
  })

  return {
    sessionId,
    stop: () => {
      running = false
      // Note: You'll need to implement a stop_streaming command in the backend
      // For now, just stop rendering
    },
    canvas,
    getFrameInfo: () => {
      if (!lastFrameInfo) return null
      const elapsed = (Date.now() - startTime) / 1000
      const fps = frameCount / elapsed
      return {
        frameId: lastFrameInfo.frameId,
        fps: Math.round(fps * 10) / 10,
        width: lastFrameInfo.width,
        height: lastFrameInfo.height,
      }
    },
  }
}

/**
 * Creates a data URL (base64) from a frame for use in img tags or downloading.
 *
 * @param frame - The frame to convert
 * @returns A data URL string
 */
export function frameToDataURL(frame: FrameEvent): string {
  const canvas = document.createElement('canvas')
  canvas.width = frame.width
  canvas.height = frame.height
  renderFrameToCanvas(canvas, frame)
  return canvas.toDataURL('image/png')
}

/**
 * Downloads a frame as an image file.
 *
 * @param frame - The frame to download
 * @param filename - The filename to save as (default: 'frame.png')
 */
export function downloadFrame(frame: FrameEvent, filename = 'frame.png'): void {
  const dataURL = frameToDataURL(frame)
  const link = document.createElement('a')
  link.href = dataURL
  link.download = filename
  link.click()
}
