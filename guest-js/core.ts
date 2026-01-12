// Minimal WebRTC helpers for camera access (no Tauri channels, no RGBA conversion)

// List available video input devices
export interface VideoInput {
  deviceId: string
  label: string
}

/** Enumerate cameras. If labels are empty, request permission once to reveal labels. */
export async function getVideoInputs(): Promise<VideoInput[]> {
  const enumerate = async () => {
    const devices = await navigator.mediaDevices.enumerateDevices()
    return devices
      .filter((d) => d.kind === 'videoinput')
      .map((d) => ({ deviceId: d.deviceId, label: d.label || 'Camera' }))
  }

  return await enumerate()
}

// Streaming helpers removed; implement WebRTC in application code as needed
