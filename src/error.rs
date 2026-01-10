use serde::{ser::Serializer, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error(transparent)]
  Io(#[from] std::io::Error),
  #[cfg(mobile)]
  #[error(transparent)]
  PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),
  #[error("Camera error: {0}")]
  CameraError(String),
  #[error("Device not found: {0}")]
  DeviceNotFound(String),
  #[error("Streaming already active for device: {0}")]
  StreamingAlreadyActive(String),
  #[error("No active stream for device: {0}")]
  NoActiveStream(String),
  #[error("Failed to initialize camera: {0}")]
  InitializationFailed(String),
  #[error("Channel send error")]
  ChannelSendError,
}

impl Serialize for Error {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(self.to_string().as_ref())
  }
}
