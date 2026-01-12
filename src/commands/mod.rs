pub mod camera;
pub mod streaming;
pub mod webrtc;

// Re-export WebRTCManager for state management
pub use camera::*;
pub use streaming::*;
pub use webrtc::*;
