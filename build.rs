const COMMANDS: &[&str] = &[
    "request-camera-permission",
    "get-available-cameras",
    "start_streaming",
    "stop_streaming",
    "capture-frame",
    "get-stream-format",
    "initialize",
    "get_available_cameras",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
