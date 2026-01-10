# Tauri Plugin Camera

A Tauri plugin for camera management with real-time streaming capabilities. Built on top of [CrabCamera](https://github.com/your-repo/crabcamera) for cross-platform camera access.

## Features

- 📷 **Camera Device Enumeration** - List all available cameras
- 🎥 **Real-time Streaming** - High-performance video streaming using Tauri Channels
- 🔄 **Format Conversion** - Automatic conversion from YUV/NV12 to RGB8
- 🎨 **Canvas Rendering** - Built-in utilities for displaying video in HTML canvas
- 📸 **Frame Capture** - Capture and download individual frames
- 🔐 **Permission Management** - Request and check camera permissions
- 🚀 **Cross-platform** - Works on Windows, macOS, Linux, iOS, and Android

## Installation

### 1. Install the plugin

```bash
# Using npm
npm install tauri-plugin-camera-api

# Using pnpm
pnpm add tauri-plugin-camera-api

# Using yarn
yarn add tauri-plugin-camera-api
```

### 2. Add the plugin to your Tauri app

In your `src-tauri/src/lib.rs`:

```rust
fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_camera::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 3. Configure permissions

Add the plugin permission to your `src-tauri/capabilities/default.json`:

```json
{
  "permissions": [
    "camera:default",
    "camera:allow-request_camera_permission",
    "camera:allow-get_available_cameras",
    "camera:allow-start_streaming",
    "camera:allow-initialize"
  ]
}
```

## Quick Start

### Basic Streaming Example

```typescript
import { initialize, createCameraStream } from 'tauri-plugin-camera-api'

// 1. Initialize the camera system
await initialize()

// 2. Get the canvas element
const canvas = document.getElementById('camera-preview') as HTMLCanvasElement

// 3. Start streaming (easiest method!)
const stream = await createCameraStream(canvas, '0', {
  flipHorizontal: true, // Mirror effect (useful for webcams)
})

// 4. Display FPS info
setInterval(() => {
  const info = stream.getFrameInfo()
  if (info) {
    console.log(`${info.fps} FPS - ${info.width}x${info.height}`)
  }
}, 1000)

// 5. Stop streaming when done
// stream.stop()
```

### Complete HTML Example

```html
<!DOCTYPE html>
<html>
  <head>
    <title>Camera Preview</title>
    <style>
      body {
        font-family: Arial, sans-serif;
        max-width: 800px;
        margin: 50px auto;
        padding: 20px;
      }

      #camera-preview {
        border: 2px solid #333;
        border-radius: 8px;
        max-width: 100%;
        background: #000;
      }

      .controls {
        margin-top: 20px;
        display: flex;
        gap: 10px;
      }

      button {
        padding: 10px 20px;
        font-size: 16px;
        border: none;
        border-radius: 4px;
        background: #007bff;
        color: white;
        cursor: pointer;
      }

      button:hover {
        background: #0056b3;
      }

      button:disabled {
        background: #ccc;
        cursor: not-allowed;
      }

      #info {
        margin-top: 10px;
        padding: 10px;
        background: #f0f0f0;
        border-radius: 4px;
        font-family: monospace;
      }
    </style>
  </head>
  <body>
    <h1>📷 Camera Preview</h1>

    <canvas id="camera-preview"></canvas>

    <div id="info">
      <div id="fps-info">FPS: --</div>
      <div id="resolution-info">Resolution: --</div>
      <div id="frame-info">Frame: --</div>
    </div>

    <div class="controls">
      <button id="start-btn">Start Camera</button>
      <button id="stop-btn" disabled>Stop Camera</button>
      <button id="capture-btn" disabled>Capture Photo</button>
      <select id="camera-select">
        <option value="">Select a camera...</option>
      </select>
    </div>

    <script type="module">
      import {
        initialize,
        getAvailableCameras,
        createCameraStream,
        downloadFrame,
      } from 'tauri-plugin-camera-api'

      let stream = null
      let currentFrame = null
      let statsInterval = null

      // Populate camera list
      async function loadCameras() {
        await initialize()
        const cameras = await getAvailableCameras()
        const select = document.getElementById('camera-select')

        cameras.forEach((camera) => {
          const option = document.createElement('option')
          option.value = camera.id
          option.textContent = `${camera.name} (${camera.platform})`
          select.appendChild(option)
        })

        if (cameras.length > 0) {
          select.value = cameras[0].id
        }
      }

      // Start camera
      document.getElementById('start-btn').addEventListener('click', async () => {
        const select = document.getElementById('camera-select')
        const deviceId = select.value

        if (!deviceId) {
          alert('Please select a camera')
          return
        }

        try {
          const canvas = document.getElementById('camera-preview')

          stream = await createCameraStream(canvas, deviceId, {
            flipHorizontal: true,
            onFrame: (frame) => {
              currentFrame = frame
            },
            onError: (error) => {
              console.error('Streaming error:', error)
            },
          })

          // Update UI
          document.getElementById('start-btn').disabled = true
          document.getElementById('stop-btn').disabled = false
          document.getElementById('capture-btn').disabled = false

          // Start stats display
          statsInterval = setInterval(() => {
            const info = stream?.getFrameInfo()
            if (info) {
              document.getElementById('fps-info').textContent = `FPS: ${info.fps}`
              document.getElementById('resolution-info').textContent =
                `Resolution: ${info.width}x${info.height}`
              document.getElementById('frame-info').textContent =
                `Frame: ${info.frameId}`
            }
          }, 100)
        } catch (error) {
          alert(`Failed to start camera: ${error}`)
        }
      })

      // Stop camera
      document.getElementById('stop-btn').addEventListener('click', () => {
        if (stream) {
          stream.stop()
          stream = null
          currentFrame = null
        }

        if (statsInterval) {
          clearInterval(statsInterval)
          statsInterval = null
        }

        // Update UI
        document.getElementById('start-btn').disabled = false
        document.getElementById('stop-btn').disabled = true
        document.getElementById('capture-btn').disabled = true
        document.getElementById('fps-info').textContent = 'FPS: --'
        document.getElementById('resolution-info').textContent = 'Resolution: --'
        document.getElementById('frame-info').textContent = 'Frame: --'
      })

      // Capture photo
      document.getElementById('capture-btn').addEventListener('click', () => {
        if (currentFrame) {
          const timestamp = new Date().toISOString().replace(/[:.]/g, '-')
          downloadFrame(currentFrame, `photo-${timestamp}.png`)
        }
      })

      // Load cameras on startup
      loadCameras()
    </script>
  </body>
</html>
```

## API Reference

### Camera Management

#### `initialize(): Promise<string>`

Initialize the camera system. Must be called before using any camera functions.

```typescript
await initialize()
```

#### `getAvailableCameras(): Promise<CameraDeviceInfo[]>`

List all available camera devices.

```typescript
const cameras = await getAvailableCameras()
cameras.forEach(camera => {
  console.log(`${camera.name} - ${camera.id}`)
})
```

#### `requestCameraPermission(): Promise<PermissionInfo>`

Request camera permission from the system (mainly for mobile).

```typescript
const permission = await requestCameraPermission()
if (permission.status === 'Granted') {
  console.log('Camera permission granted')
}
```

### Streaming

#### `startStreaming(deviceId: string, onFrame: (frame: FrameEvent) => void): Promise<string>`

Start streaming from a camera device. Returns a session ID.

```typescript
const sessionId = await startStreaming('0', (frame) => {
  console.log(`Received frame: ${frame.width}x${frame.height}`)
  // Process frame...
})
```

#### `createCameraStream(canvas: HTMLCanvasElement, deviceId: string, options?: StreamOptions): Promise<StreamController>`

High-level API that automatically renders frames to a canvas.

```typescript
const stream = await createCameraStream(canvas, '0', {
  autoResize: true,          // Auto-resize canvas to frame size
  flipHorizontal: true,      // Mirror effect
  flipVertical: false,       // Flip vertically
  onFrame: (frame) => {      // Optional callback
    console.log(`Frame ${frame.frameId}`)
  },
  onError: (error) => {      // Optional error handler
    console.error(error)
  },
})

// Get stream info
const info = stream.getFrameInfo()
console.log(`${info.fps} FPS`)

// Stop streaming
stream.stop()
```

### Rendering Utilities

#### `renderFrameToCanvas(canvas: HTMLCanvasElement, frame: FrameEvent, options?: RenderOptions): void`

Manually render a frame to a canvas element.

```typescript
renderFrameToCanvas(canvas, frame, {
  autoResize: true,
  flipHorizontal: true,
  flipVertical: false,
})
```

#### `frameToDataURL(frame: FrameEvent): string`

Convert a frame to a data URL (base64).

```typescript
const dataURL = frameToDataURL(frame)
document.getElementById('img').src = dataURL
```

#### `downloadFrame(frame: FrameEvent, filename?: string): void`

Download a frame as an image file.

```typescript
downloadFrame(frame, 'photo.png')
```

## TypeScript Types

```typescript
interface CameraDeviceInfo {
  id: string
  name: string
  description?: string
  isAvailable: boolean
  supportsFormats: CameraFormat[]
  platform: Platform
}

interface CameraFormat {
  width: number
  height: number
  fps: number
  formatType: string
}

interface FrameEvent {
  frameId: number
  data: Uint8Array      // RGB8 format (3 bytes per pixel)
  width: number
  height: number
  timestampMs: number
  format: string        // Always "RGB8"
}

interface PermissionInfo {
  status: PermissionStatus
  message: string
  canRequest: boolean
}

enum PermissionStatus {
  Granted = 'Granted',
  Denied = 'Denied',
  NotDetermined = 'NotDetermined',
  Restricted = 'Restricted',
}
```

## Advanced Examples

### Custom Frame Processing

```typescript
import { startStreaming } from 'tauri-plugin-camera-api'

await startStreaming('0', (frame) => {
  // Access raw RGB8 data
  const data = frame.data

  // Example: Calculate average brightness
  let sum = 0
  for (let i = 0; i < data.length; i += 3) {
    const r = data[i]
    const g = data[i + 1]
    const b = data[i + 2]
    sum += (r + g + b) / 3
  }
  const avgBrightness = sum / (data.length / 3)
  console.log(`Brightness: ${avgBrightness}`)
})
```

### Multiple Camera Streams

```typescript
const canvas1 = document.getElementById('camera1') as HTMLCanvasElement
const canvas2 = document.getElementById('camera2') as HTMLCanvasElement

const cameras = await getAvailableCameras()

const stream1 = await createCameraStream(canvas1, cameras[0].id)
const stream2 = await createCameraStream(canvas2, cameras[1].id)
```

### React Integration

```tsx
import { useEffect, useRef, useState } from 'react'
import { initialize, createCameraStream } from 'tauri-plugin-camera-api'

function CameraPreview({ deviceId }: { deviceId: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [fps, setFps] = useState(0)
  const [stream, setStream] = useState<any>(null)

  useEffect(() => {
    let mounted = true

    async function start() {
      await initialize()

      if (!canvasRef.current || !mounted) return

      const s = await createCameraStream(canvasRef.current, deviceId, {
        flipHorizontal: true,
      })

      setStream(s)

      // Update FPS
      const interval = setInterval(() => {
        const info = s.getFrameInfo()
        if (info && mounted) {
          setFps(info.fps)
        }
      }, 500)

      return () => {
        mounted = false
        clearInterval(interval)
        s.stop()
      }
    }

    start()
  }, [deviceId])

  return (
    <div>
      <canvas ref={canvasRef} />
      <div>FPS: {fps}</div>
    </div>
  )
}
```

## Performance Tips

1. **Use `createCameraStream()`** - It's optimized for rendering and handles frame management automatically
2. **Channel vs Events** - This plugin uses Tauri Channels (not events) for better performance with large data
3. **Canvas Rendering** - The Canvas 2D API is very efficient for real-time video rendering
4. **Frame Processing** - For heavy processing, consider using Web Workers to avoid blocking the UI thread

## Platform Support

| Platform | Status | Notes |
|----------|--------|-------|
| Windows  | ✅ | DirectShow/Media Foundation |
| macOS    | ✅ | AVFoundation |
| Linux    | ✅ | V4L2 |
| iOS      | ✅ | AVFoundation |
| Android  | ✅ | Camera2 API |

## Troubleshooting

### No cameras found

- **Windows**: Check device manager for camera drivers
- **macOS/iOS**: Grant camera permission in System Preferences
- **Linux**: Ensure your user is in the `video` group: `sudo usermod -a -G video $USER`

### Black screen

- Check camera permissions
- Try a different camera device
- Verify the camera isn't being used by another application

### Low FPS

- Check `getFrameInfo()` to see actual FPS
- Consider reducing resolution if needed
- Ensure no heavy processing in the `onFrame` callback

## Contributing

Contributions are welcome! Please read our [contributing guide](CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Tauri](https://tauri.app/)
- Camera access powered by [CrabCamera](https://github.com/your-repo/crabcamera)
