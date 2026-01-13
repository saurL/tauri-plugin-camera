# Tauri Plugin Camera

A Tauri plugin for camera management with WebRTC streaming capabilities. Built on top of [CrabCamera](https://github.com/Michael-A-Kuykendall/crabcamera) for cross-platform camera access.

> ⚠️ **Early Development Notice**
>
> This plugin is currently in early development and has only been tested on **Windows**. The video quality and format support are limited at this stage:
>
> - Currently supports only **H.264** format
> - Video quality optimization is ongoing
> - Limited format configuration options
>
> **We welcome contributions!**
>
> - Testing on other platforms (macOS, Linux, iOS)
> - Camera compatibility reports
> - Pull requests for additional video formats and quality improvements
> - Performance optimization suggestions
> - My golal isn't to make it work on android/ios right now but open to contributing PRs for mobile support.

## Platform Support

| Platform | Status |
| -------- | ------ |
| Windows  | ✅     |
| macOS    | ❓     |
| Linux    | ❓     |
| iOS      | ❌     |
| Android  | ❌     |

## Supported Formats

| Type          | Format                     | Status | Notes                                        |
| ------------- | -------------------------- | ------ | -------------------------------------------- |
| Camera input  | NV12 (YUV 4:2:0)           | ✅     | Preferred on Windows (Media Foundation).     |
| Camera input  | I420 / YUV420p             | ❌     | Converted and encoded to H.264.              |
| Camera input  | MJPEG                      | ❌     | Not tested yet.                              |
| Camera input  | YUY2 (YUYV, YUV422)        | ❌     | Common USB webcams; conversion required.     |
| Camera input  | UYVY (YUV422)              | ❌     | Requires conversion to I420/NV12.            |
| Camera input  | YV12 (YUV420p, V before U) | ❌     | Similar to I420; plane order differs.        |
| Camera input  | NV21 (YUV 4:2:0)           | ❌     | Android-oriented; not currently targeted.    |
| Camera input  | H.264 (UVC cameras)        | ✅     | Some webcams output H.264 directly.          |
| Camera input  | RGB24                      | ❓     | Less common; conversion to I420 is required. |
| WebRTC output | H.264 (AVC)                | ✅     | `video/h264` track attached.                 |
| WebRTC output | VP8 / VP9                  | ❌     | Not used currently.                          |
| Audio         | —                          | ❌     | Audio tracks not supported yet.              |

## Installation

### 1. Install the plugin API

```bash
# Using npm
npm install saurL/tauri-plugin-camera-api

# Using pnpm
pnpm add saurL/tauri-plugin-camera-api

# Using yarn
yarn add saurL/tauri-plugin-camera-api
```

### 2. Install rust dependency

Add the plugin to your `src-tauri/Cargo.toml`:

```toml
[dependencies]
tauri-plugin-camera = {git = "https://github.com/saurL/tauri-plugin-camera.git"}
```

### 3. Add the plugin to your Tauri app

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
  "permissions": ["camera:default"]
}
```

## Examples

A complete WebRTC streaming example is available in the [`examples/minimal-streaming`](examples/minimal-streaming) directory. This example demonstrates:

- Real-time camera streaming via WebRTC
- Selecting between available cameras
- H.264 video encoding
- Proper cleanup on disconnect

**To run the example:**

```bash
cd examples/minimal-streaming
pnpm install
pnpm tauri dev --release # For camera performance otherwise you would get latency issues
```

## API Reference

### Camera Management

#### `initialize(): Promise<string>`

Initialize the camera system. Must be called before using any camera functions.

```typescript
await initialize();
```

#### `getAvailableCameras(): Promise<CameraDeviceInfo[]>`

List all available camera devices.

```typescript
const cameras = await getAvailableCameras();
cameras.forEach((camera) => {
  console.log(`${camera.name} - ${camera.id}`);
});
```

#### `requestCameraPermission(): Promise<PermissionInfo>`

Request camera permission from the system (mainly for mobile).

```typescript
const permission = await requestCameraPermission();
if (permission.status === "Granted") {
  console.log("Camera permission granted");
}
```

### Streaming

#### `startStreaming(deviceId: string, onFrame: (frame: FrameEvent) => void): Promise<string>`

Start streaming from a camera device. Returns a session ID.

```typescript
const sessionId = await startStreaming("0", (frame) => {
  console.log(`Received frame: ${frame.width}x${frame.height}`);
  // Process frame...
});
```

#### `createCameraStream(canvas: HTMLCanvasElement, deviceId: string, options?: StreamOptions): Promise<StreamController>`

High-level API that automatically renders frames to a canvas.

```typescript
const stream = await createCameraStream(canvas, "0", {
  autoResize: true, // Auto-resize canvas to frame size
  flipHorizontal: true, // Mirror effect
  flipVertical: false, // Flip vertically
  onFrame: (frame) => {
    // Optional callback
    console.log(`Frame ${frame.frameId}`);
  },
  onError: (error) => {
    // Optional error handler
    console.error(error);
  },
});

// Get stream info
const info = stream.getFrameInfo();
console.log(`${info.fps} FPS`);

// Stop streaming
stream.stop();
```

## TypeScript Types

## Rust-side Streaming (without WebRTC)

You can start a camera stream and consume raw frames directly from Rust without creating a WebRTC connection. This uses the plugin state to open the camera, then reads frames from a `watch::Receiver` of `FrameEvent`.

```rust
use tauri::AppHandle;
use tauri_plugin_camera::{CameraExt, Result};

pub async fn stream_and_process(handle: AppHandle, device_id: &str) -> Result<()> {
  let camera = handle.camera();

  // 1) Start streaming the camera (returns a session_id)
  let _session_id = camera.start_streaming(device_id.to_string()).await?;

  // 2) Get a frame receiver for this device
  let mut rx = camera.get_receiver_by_device_id(device_id).await?;

  // 3) Consume frames as they arrive
  while rx.changed().await.is_ok() {
    let frame = rx.borrow().clone();
    if let Some(frame) = frame {
      // frame.data is raw YUV/RGB (depending on platform);
      // width/height/format describe the buffer layout.
      do_something_with(frame);
    }
  }

  Ok(())
}

fn do_something_with(_frame: tauri_plugin_camera::models::FrameEvent) {
  // e.g., feed into an encoder, write to disk, run CV, etc.
}
```

Key points:

- `start_streaming(device_id)` opens the camera and starts pushing frames you can call it even if a stream is already active for that device.
- `get_receiver_by_device_id(device_id)` returns a `watch::Receiver<Option<FrameEvent>>` you can await on.
- Frames are delivered on the Rust side; you can process, transcode, or forward them as needed.
- Call `stop_streaming(session_id)` when done to release the camera.

```typescript
interface CameraDeviceInfo {
  id: string;
  name: string;
  description?: string;
  isAvailable: boolean;
  supportsFormats: CameraFormat[];
  platform: Platform;
}

interface CameraFormat {
  width: number;
  height: number;
  fps: number;
  formatType: string;
}

interface FrameEvent {
  frameId: number;
  data: Uint8Array; // RGB8 format (3 bytes per pixel)
  width: number;
  height: number;
  timestampMs: number;
  format: string; // Always "RGB8"
}

interface PermissionInfo {
  status: PermissionStatus;
  message: string;
  canRequest: boolean;
}

enum PermissionStatus {
  Granted = "Granted",
  Denied = "Denied",
  NotDetermined = "NotDetermined",
  Restricted = "Restricted",
}
```

## WebRTC Streaming Guide

### Overview

WebRTC streaming allows real-time camera video streaming with H.264 encoding. The flow involves:

1. Backend: Initialize WebRTC connection and start camera stream
2. Frontend: Create peer connection and exchange SDP offers/answers
3. Connection: Automatic linking of stream to connection for cleanup

### Flow Diagram

```
Frontend                          Backend
   |                                |
   |-- 1. startCameraWebRTCSesion -->|
   |                                |-- Create peer connection
   |<-- {offer, connectionId} -------|-- Start camera stream
   |                                |-- Register stream -> connection mapping
   |                                |
   |-- 2. createAnswer() ----------->| (WebRTC negotiation)
   |-- 3. setRemoteDescription() -->|
   |                                |-- H.264 frames flow
   |<-- ontrack event --------------|
   |-- 4. Display video -------------|
   |                                |
   |-- 5. closeConnection() ------->|
   |                                |-- Stop stream (automatic)
   |                                |-- Close connection
   |<-- ✅ success --------------|

```

### Key Components

#### 1. **Imports**

```typescript
import {
  getAvailableCameras,
  startCameraWebRTCSesion, // Start WebRTC session with camera
  setRemoteDescription, // Send answer back to backend
  closeConnection, // Clean up connection & stream
  type CameraDeviceInfo,
} from "tauri-plugin-camera-api";
```

#### 2. **State Management**

```typescript
// Keep track of connection and peer connection
let currentConnectionId: string | null = null;
let peerConnection: RTCPeerConnection | null = null;
let videoElement: HTMLVideoElement | null = null;
```

### Critical Steps Summary

1. **Load cameras** → `getAvailableCameras()`
2. **Start session** → `startCameraWebRTCSesion(deviceId)` returns `{offer, connectionId}`
3. **Setup peer connection** → Create `RTCPeerConnection`
4. **Exchange SDP** → Set remote description (offer) → Create answer → Send back via `setRemoteDescription()`
5. **Handle video** → Listen to `ontrack` event and set video element's `srcObject`
6. **Cleanup** → Call `closeConnection()` which automatically stops the stream

### Important Notes

- ✅ **Auto cleanup**: When you call `closeConnection()`, the backend automatically stops the linked stream
- ✅ **Error handling**: Always wrap async calls in try/catch
- ✅ **State tracking**: Keep refs to `connectionId`, `peerConnection`, and video element
- ✅ **Cleanup on unmount**: In React/Vue, ensure cleanup on component unmount (close peer connection, stop video tracks, close connection)
- ⚠️ **Permissions**: Camera permissions must be granted at OS level before calling these functions

## Contributing

Contributions are welcome!

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Tauri](https://tauri.app/)
- Camera access powered by [CrabCamera](https://github.com/your-repo/crabcamera)
