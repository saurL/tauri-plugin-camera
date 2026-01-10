const COMMANDS: &[&str] = &[
    "request-camera-permission",
    "start-streaming",
    "stop-streaming",
    "initialize",
    "get-available-cameras",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
