const COMMANDS: &[&str] = &[
    "request_camera_permission",
    "start_streaming",
    "stop_streaming",
    "initialize",
    "get_available_cameras",
    "create_offer",
    "create_answer",
    "set_remote_description",
    "add_ice_candidate",
    "close_connection",
    "get_connection_state",
    "start_camera_webrtc_session",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
