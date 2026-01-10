const COMMANDS: &[&str] = &[
    "request-camera-permission",
    "get-available-cameras",
    "start-streaming",
    "stop-streaming",
    "capture-frame",
    "get-stream-format",
    "initialize",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
