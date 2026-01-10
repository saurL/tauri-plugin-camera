const COMMANDS: &[&str] = &[
    "request_camera_permission",
    "start_streaming",
    "stop_streaming",
    "initialize",
    "get_available_cameras",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
