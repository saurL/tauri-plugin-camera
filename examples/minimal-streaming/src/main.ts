import { getAvailableCameras, startCameraWebRTCSesion, closeConnection, setRemoteDescription, addIceCandidate, type CameraDeviceInfo } from "tauri-plugin-camera-api";

let currentConnectionId: string | null = null;
let peerConnection: RTCPeerConnection | null = null;
let videoElement: HTMLVideoElement | null = null;

async function listCameras() {
  const cameraSelect = document.querySelector("#camera-select") as HTMLSelectElement;
  const cameraList = document.querySelector("#camera-list") as HTMLDivElement;
  const status = document.querySelector("#status") as HTMLDivElement;

  try {
    status.textContent = "Loading cameras...";

    const devices = await getAvailableCameras();

    cameraSelect.innerHTML = '<option value="">Select a camera...</option>';
    cameraList.innerHTML = "";

    if (devices.length === 0) {
      status.textContent = "No cameras found";
      return;
    }

    devices.forEach((device: CameraDeviceInfo) => {
      const option = document.createElement("option");
      option.value = device.id;
      option.textContent = `${device.name} (${device.platform})`;
      cameraSelect.appendChild(option);

      const deviceEl = document.createElement("div");
      deviceEl.className = "camera-item";
      deviceEl.innerHTML = `
        <strong>${device.name}</strong><br>
        <small>${device.description} - ID: ${device.id}</small><br>
        <small>Formats: ${device.supports_formats.map(f => `${f.width}x${f.height}@${f.fps}fps`).join(', ')}</small>
      `;
      cameraList.appendChild(deviceEl);
    });

    cameraSelect.disabled = false;
    status.textContent = `Found ${devices.length} camera(s). Select one to start streaming.`;
  } catch (error) {
    status.textContent = `Error: ${error}`;
    console.error(error);
}}

async function startStream() {
  const cameraSelect = document.querySelector("#camera-select") as HTMLSelectElement;
  const startBtn = document.querySelector("#start-stream") as HTMLButtonElement;
  const stopBtn = document.querySelector("#stop-stream") as HTMLButtonElement;
  const videoContainer = document.querySelector(".video-container") as HTMLDivElement;
  const status = document.querySelector("#status") as HTMLDivElement;

  const deviceId = cameraSelect.value;
  if (!deviceId) {
    alert("Please select a camera first");
    return;
  }

  try {
    startBtn.disabled = true;
    status.textContent = "Starting stream...";

    // Create video element if it doesn't exist
    if (!videoElement) {
      videoElement = document.createElement("video");
      videoElement.id = "camera-video";
      videoElement.autoplay = true;
      videoElement.playsInline = true;
      videoElement.style.maxWidth = "100%";
      videoElement.style.border = "2px solid #ccc";
      videoElement.style.borderRadius = "8px";
      videoElement.style.background = "#000";

      // Remove canvas if exists
      const canvas = document.querySelector("#camera-canvas");
      if (canvas) {
        canvas.remove();
      }

      videoContainer.insertBefore(videoElement, videoContainer.firstChild);
    }

    // Start WebRTC session on backend
    const { offer, connectionId } = await startCameraWebRTCSesion(deviceId);
    currentConnectionId = connectionId;

    // Create peer connection
    peerConnection = new RTCPeerConnection();

    // Handle incoming tracks
    peerConnection.ontrack = (event) => {
      if (videoElement && event.streams[0]) {
        videoElement.srcObject = event.streams[0];
        status.textContent = "Stream connected successfully!";
      }
    };

    // Set remote description (the offer from backend)
    await peerConnection.setRemoteDescription({
      type: 'offer' ,
      sdp: offer.sdp
    });

    // Handle ICE candidates from local peer
    peerConnection.onicecandidate = async (event) => {
      if (event.candidate) {
        await addIceCandidate(currentConnectionId!, {
          candidate: event.candidate.candidate,
          sdpMid: event.candidate.sdpMid!,
          sdpMlineIndex: event.candidate.sdpMLineIndex!,
        });
      }
    };

    // Create answer
    const answer = await peerConnection.createAnswer();
    await peerConnection.setLocalDescription(answer);

    // Send answer back to backend using the Tauri command
    await setRemoteDescription(connectionId, {
      type: answer.type as 'offer' | 'answer',
      sdp: answer.sdp || '',
    });

    cameraSelect.disabled = true;
    stopBtn.disabled = false;
  } catch (error) {
    status.textContent = `Error: ${error}`;
    console.error(error);
    startBtn.disabled = false;
  }
}

async function stopStream() {
  const startBtn = document.querySelector("#start-stream") as HTMLButtonElement;
  const stopBtn = document.querySelector("#stop-stream") as HTMLButtonElement;
  const cameraSelect = document.querySelector("#camera-select") as HTMLSelectElement;
  const status = document.querySelector("#status") as HTMLDivElement;

  if (!currentConnectionId) return;

  try {
    stopBtn.disabled = true;
    status.textContent = "Stopping stream...";

    // Close peer connection
    if (peerConnection) {
      peerConnection.close();
      peerConnection = null;
    }

    // Stop video element
    if (videoElement && videoElement.srcObject) {
      const stream = videoElement.srcObject as MediaStream;
      stream.getTracks().forEach((track) => track.stop());
      videoElement.srcObject = null;
    }

    // Close backend connection
    await closeConnection(currentConnectionId);

    currentConnectionId = null;
    startBtn.disabled = false;
    cameraSelect.disabled = false;
    status.textContent = "Stream stopped";
  } catch (error) {
    status.textContent = `Error: ${error}`;
    console.error(error);
  } finally {
    stopBtn.disabled = true;
  }
}

window.addEventListener("DOMContentLoaded", () => {
  document.querySelector("#start-stream")?.addEventListener("click", startStream);
  document.querySelector("#stop-stream")?.addEventListener("click", stopStream);

  document.querySelector("#camera-select")?.addEventListener("change", (e) => {
    const select = e.target as HTMLSelectElement;
    const startBtn = document.querySelector("#start-stream") as HTMLButtonElement;
    startBtn.disabled = !select.value;
  });

  // Auto-load cameras on startup
  listCameras();
});
