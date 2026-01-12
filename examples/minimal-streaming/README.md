# Camera Streaming - Minimal Example

This is a minimal example demonstrating how to use the `tauri-plugin-camera` for real-time camera streaming.

## Features

- List all available camera devices
- Select a camera from the list
- Start/stop video streaming
- Display live camera feed on a canvas

## Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [Node.js](https://nodejs.org/)
- [pnpm](https://pnpm.io/installation)

## Setup

1. Install dependencies:

```bash
pnpm install
```

## Running the Example

### Development Mode

```bash
pnpm tauri dev
```

> ⚠️ **Performance Note**: For optimal camera performance with minimal latency, it's recommended to use **release mode** during development:
>
> ```bash
> pnpm tauri dev --release
> ```
>
> Release builds have significant performance improvements for real-time video processing. Debug builds may experience noticeable camera latency.

### Build for Production

```bash
pnpm tauri build
```

## How It Works

### Frontend (TypeScript)

The frontend uses Tauri's `Channel` API to receive streaming frames from the backend:

```typescript
import { invoke, Channel } from "@tauri-apps/api/core";

// Create a channel to receive frames
const onFrame = new Channel<FrameEvent>();
onFrame.onmessage = (frame: FrameEvent) => {
  // Render frame to canvas
  const imageData = ctx.createImageData(frame.width, frame.height);
  const data = new Uint8Array(frame.data);

  for (let i = 0; i < data.length; i++) {
    imageData.data[i] = data[i];
  }

  ctx.putImageData(imageData, 0, 0);
};

// Start streaming
const streamId = await invoke<string>("plugin:camera|stream_camera", {
  deviceId: "0",
  onFrame,
});
```

### Backend (Rust)

The plugin uses CrabCamera to handle camera access and streaming. The backend automatically:

1. Lists available camera devices
2. Initializes the selected camera with optimal settings
3. Streams frames to the frontend via Tauri channels
4. Manages camera lifecycle (start/stop)

### Key Commands

- `list_devices`: Returns all available camera devices
- `stream_camera`: Starts streaming from a specific camera
- `stop_stream`: Stops an active stream

## Project Structure

```
minimal-streaming/
├── src/               # Frontend code
│   ├── main.ts        # Main TypeScript logic
│   └── styles.css     # CSS styles
├── src-tauri/         # Backend code
│   ├── src/
│   │   └── lib.rs     # Rust application entry point
│   ├── Cargo.toml     # Rust dependencies (includes tauri-plugin-camera)
│   └── capabilities/
│       └── default.json  # Permissions configuration
├── index.html         # Main HTML file
└── README.md          # This file
```

## Permissions

The app requires the `camera:default` permission set, which includes:

- `allow-list-devices`: List available cameras
- `allow-stream-camera`: Start camera streaming
- `allow-stop-stream`: Stop camera streaming

These are configured in [src-tauri/capabilities/default.json](src-tauri/capabilities/default.json).

## Troubleshooting

### No cameras found

- Ensure you have a working camera connected
- Check that your OS hasn't blocked camera access
- Try running with administrator/sudo privileges

### Stream not displaying

- Check the browser console for errors
- Verify that the canvas element is properly initialized
- Ensure the camera is not being used by another application

### Build errors

- Make sure the parent plugin directory (`../../..`) contains the latest `tauri-plugin-camera` code
- Run `cargo clean` in the `src-tauri` directory and rebuild
- Check that all dependencies are up to date with `pnpm install`

## Learn More

- [Tauri Documentation](https://tauri.app/)
- [CrabCamera Library](https://github.com/l1npengtul/crabcamera)
- [Plugin Development Guide](../../../CLAUDE.md)
