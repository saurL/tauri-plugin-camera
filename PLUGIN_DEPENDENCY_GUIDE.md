# CrabCamera Plugin Dependency Guide

Guide pour utiliser CrabCamera comme dépendance dans votre propre plugin Tauri.

---

## Installation dans Votre Plugin

### 1. Ajouter CrabCamera à votre `Cargo.toml`

```toml
[package]
name = "votre-plugin"
version = "0.1.0"
edition = "2021"

[dependencies]
# CrabCamera comme dépendance
crabcamera = { path = "../crabcamera" }  # Chemin local
# Ou depuis git:
# crabcamera = { git = "https://github.com/votre-repo/crabcamera", branch = "main" }

# Dépendances nécessaires
tauri = "2.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.40", features = ["full"] }
```

### 2. Activer les Features selon vos Besoins

```toml
[dependencies.crabcamera]
path = "../crabcamera"
features = [
    "audio",      # Si vous voulez l'audio
    "recording",  # Si vous voulez l'enregistrement vidéo
]
```

---

## Importer et Utiliser les Modules CrabCamera

### Structure des Re-exports de CrabCamera

CrabCamera expose ces modules publics :

```rust
// Types principaux
pub use crabcamera::{
    CameraError,           // Gestion d'erreurs
    CameraSystem,          // Système de gestion des caméras
    PlatformCamera,        // Caméra platform-specific
    CameraDeviceInfo,      // Info sur un device
    CameraFormat,          // Format vidéo
    CameraFrame,           // Frame capturée
    CameraInitParams,      // Paramètres d'initialisation
    FrameMetadata,         // Métadonnées de frame
    Platform,              // Enum de plateforme
};
```

---

## Exemples d'Utilisation dans Votre Plugin

### Exemple 1 : Wrapper Simple pour Streaming

Créez votre propre commande Tauri qui utilise CrabCamera :

```rust
// votre-plugin/src/lib.rs

use crabcamera::{
    CameraSystem, PlatformCamera, CameraInitParams,
    CameraFrame, CameraError
};
use std::sync::Arc;
use tauri::{command, AppHandle, Runtime};
use tokio::sync::Mutex as AsyncMutex;

// Votre propre structure de session
pub struct MyCameraSession {
    camera: Arc<AsyncMutex<PlatformCamera>>,
    device_id: String,
}

impl MyCameraSession {
    pub async fn new(device_id: String) -> Result<Self, CameraError> {
        // Utiliser CrabCamera pour créer la caméra
        let params = CameraInitParams::new(device_id.clone())
            .with_format(crabcamera::platform::optimizations::get_optimal_settings().format.unwrap());

        let camera = PlatformCamera::new(params)?;

        Ok(Self {
            camera: Arc::new(AsyncMutex::new(camera)),
            device_id,
        })
    }

    pub async fn start_streaming<F>(&self, callback: F) -> Result<(), CameraError>
    where
        F: Fn(CameraFrame) + Send + Sync + 'static,
    {
        let cam = self.camera.lock().await;
        cam.start_streaming(Box::new(callback)).await
    }

    pub async fn stop(&self) -> Result<(), CameraError> {
        let mut cam = self.camera.lock().await;
        cam.stop_stream()
    }
}

// Votre commande Tauri
#[command]
pub async fn my_start_camera(device_id: String) -> Result<String, String> {
    let session = MyCameraSession::new(device_id)
        .await
        .map_err(|e| format!("Failed to create camera: {}", e))?;

    session.start_streaming(|frame| {
        // Votre logique de traitement
        println!("Received frame: {}x{}", frame.width, frame.height);
    }).await.map_err(|e| format!("Failed to start: {}", e))?;

    Ok("Camera started".to_string())
}
```

### Exemple 2 : Wrapper avec Gestion de Multiple Caméras

```rust
// votre-plugin/src/camera_manager.rs

use crabcamera::{CameraSystem, CameraDeviceInfo, PlatformCamera, CameraInitParams};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct MultiCameraManager {
    system: CameraSystem,
    cameras: Arc<RwLock<HashMap<String, Arc<PlatformCamera>>>>,
}

impl MultiCameraManager {
    pub fn new() -> Self {
        Self {
            system: CameraSystem::new(),
            cameras: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Liste toutes les caméras disponibles
    pub fn list_cameras(&self) -> Result<Vec<CameraDeviceInfo>, String> {
        self.system
            .list_devices()
            .map_err(|e| format!("Failed to list devices: {}", e))
    }

    /// Initialise une caméra
    pub async fn initialize_camera(
        &self,
        device_id: String,
    ) -> Result<(), String> {
        let mut cameras = self.cameras.write().await;

        if cameras.contains_key(&device_id) {
            return Err(format!("Camera {} already initialized", device_id));
        }

        // Utiliser les paramètres optimaux de CrabCamera
        let params = CameraInitParams::new(device_id.clone())
            .with_format(
                crabcamera::platform::optimizations::get_optimal_settings()
                    .format
                    .unwrap()
            );

        let camera = PlatformCamera::new(params)
            .map_err(|e| format!("Failed to create camera: {}", e))?;

        cameras.insert(device_id.clone(), Arc::new(camera));

        Ok(())
    }

    /// Obtient une caméra par ID
    pub async fn get_camera(&self, device_id: &str) -> Option<Arc<PlatformCamera>> {
        let cameras = self.cameras.read().await;
        cameras.get(device_id).cloned()
    }

    /// Libère une caméra
    pub async fn release_camera(&self, device_id: &str) -> Result<(), String> {
        let mut cameras = self.cameras.write().await;

        if let Some(camera) = cameras.remove(device_id) {
            // PlatformCamera se nettoie automatiquement au drop
            drop(camera);
            Ok(())
        } else {
            Err(format!("Camera {} not found", device_id))
        }
    }
}

// Commandes Tauri utilisant le manager
#[tauri::command]
pub async fn list_available_cameras(
    state: tauri::State<'_, MultiCameraManager>
) -> Result<Vec<CameraDeviceInfo>, String> {
    state.list_cameras()
}

#[tauri::command]
pub async fn init_camera(
    device_id: String,
    state: tauri::State<'_, MultiCameraManager>
) -> Result<(), String> {
    state.initialize_camera(device_id).await
}
```

### Exemple 3 : Streaming avec Tauri Channels

```rust
// votre-plugin/src/streaming.rs

use crabcamera::{PlatformCamera, CameraFrame, CameraInitParams};
use tauri::{command, ipc::Channel};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameEvent {
    pub frame_id: u64,
    #[serde(with = "serde_bytes")]
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub timestamp_ms: u64,
}

#[command]
pub async fn stream_camera(
    device_id: String,
    on_frame: Channel<FrameEvent>,
) -> Result<String, String> {
    // Créer la caméra avec CrabCamera
    let params = CameraInitParams::new(device_id.clone())
        .with_format(
            crabcamera::platform::optimizations::get_photography_format()
        );

    let camera = PlatformCamera::new(params)
        .map_err(|e| format!("Failed to create camera: {}", e))?;

    let camera_arc = Arc::new(AsyncMutex::new(camera));
    let start_time = std::time::Instant::now();
    let frame_counter = Arc::new(std::sync::atomic::AtomicU64::new(0));

    // Créer le callback de streaming
    let counter = frame_counter.clone();
    let callback = Box::new(move |frame: CameraFrame| {
        let frame_id = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let timestamp_ms = start_time.elapsed().as_millis() as u64;

        let event = FrameEvent {
            frame_id,
            data: frame.data,
            width: frame.width,
            height: frame.height,
            timestamp_ms,
        };

        if let Err(e) = on_frame.send(event) {
            log::error!("Failed to send frame: {}", e);
        }
    });

    // Démarrer le streaming
    let cam = camera_arc.lock().await;
    cam.start_streaming(callback)
        .await
        .map_err(|e| format!("Failed to start streaming: {}", e))?;

    Ok(format!("Streaming started for {}", device_id))
}
```

---

## Utilisation des Modules Platform-Specific

### Accéder aux Optimisations

```rust
use crabcamera::platform::optimizations;

// Obtenir le format optimal pour la photographie
let photo_format = optimizations::get_photography_format();

// Obtenir les paramètres optimaux complets
let optimal_params = optimizations::get_optimal_settings();

// Utiliser dans votre code
let params = CameraInitParams::new("0".to_string())
    .with_format(photo_format);
```

### Détecter la Plateforme

```rust
use crabcamera::Platform;

let platform = Platform::current();

match platform {
    Platform::Windows => println!("Running on Windows"),
    Platform::MacOS => println!("Running on macOS"),
    Platform::Linux => println!("Running on Linux"),
    Platform::Unknown => println!("Unknown platform"),
}
```

### Utiliser PlatformCamera Directement

```rust
use crabcamera::{PlatformCamera, CameraInitParams, CameraFormat};

// Créer une caméra avec paramètres personnalisés
let format = CameraFormat {
    width: 1920,
    height: 1080,
    fps: 30.0,
    format: Some("RGB24".to_string()),
};

let params = CameraInitParams::new("0".to_string())
    .with_format(format)
    .with_auto_focus(true)
    .with_auto_exposure(true);

let camera = PlatformCamera::new(params)?;

// Obtenir le format actuel
let current_format = camera.get_format();
println!("Camera format: {}x{}@{}fps",
    current_format.width,
    current_format.height,
    current_format.fps
);

// Capturer une frame
let frame = camera.capture_frame().await?;
println!("Captured frame: {} bytes", frame.data.len());

// Démarrer le streaming
camera.start_streaming(Box::new(|frame| {
    // Traiter chaque frame
    println!("Frame: {}x{}", frame.width, frame.height);
})).await?;

// Arrêter
camera.stop_stream()?;
```

---

## Gestion des Erreurs

CrabCamera utilise `CameraError` pour toutes les erreurs :

```rust
use crabcamera::CameraError;

fn handle_camera_error(error: CameraError) -> String {
    match error {
        CameraError::InitializationFailed(msg) => {
            format!("Initialization failed: {}", msg)
        }
        CameraError::StreamingFailed(msg) => {
            format!("Streaming failed: {}", msg)
        }
        CameraError::DeviceNotFound(id) => {
            format!("Device {} not found", id)
        }
        CameraError::PermissionDenied => {
            "Camera permission denied".to_string()
        }
        CameraError::FormatNotSupported(fmt) => {
            format!("Format not supported: {:?}", fmt)
        }
        _ => format!("Camera error: {:?}", error),
    }
}

// Utilisation
match PlatformCamera::new(params) {
    Ok(camera) => {
        // Succès
    }
    Err(e) => {
        let msg = handle_camera_error(e);
        return Err(msg);
    }
}
```

---

## Enregistrer Votre Plugin Tauri

Dans votre `lib.rs` principal :

```rust
// votre-plugin/src/lib.rs

use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

mod camera_manager;
mod streaming;

use camera_manager::MultiCameraManager;

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("votre-plugin")
        .invoke_handler(tauri::generate_handler![
            // Vos commandes utilisant CrabCamera
            camera_manager::list_available_cameras,
            camera_manager::init_camera,
            streaming::stream_camera,
        ])
        .setup(|app, _api| {
            // Initialiser le state global
            app.manage(MultiCameraManager::new());
            Ok(())
        })
        .build()
}
```

---

## Exemple d'Architecture Recommandée

### Structure de Fichiers

```
votre-plugin/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Point d'entrée, init du plugin
│   ├── camera_manager.rs   # Gestion des caméras (utilise CrabCamera)
│   ├── streaming.rs        # Streaming (utilise PlatformCamera)
│   ├── commands.rs         # Commandes Tauri
│   └── types.rs            # Vos types custom
```

### lib.rs

```rust
use tauri::{plugin::TauriPlugin, Runtime};

pub mod camera_manager;
pub mod streaming;
pub mod commands;
pub mod types;

// Re-exporter CrabCamera types pour vos utilisateurs
pub use crabcamera::{
    CameraError, CameraFormat, CameraFrame, CameraInitParams,
};

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("votre-plugin")
        .invoke_handler(tauri::generate_handler![
            commands::list_cameras,
            commands::init_camera,
            commands::start_streaming,
            commands::stop_streaming,
        ])
        .build()
}
```

### commands.rs

```rust
use crate::camera_manager::MultiCameraManager;
use crate::types::*;
use tauri::{command, State};

#[command]
pub async fn list_cameras(
    state: State<'_, MultiCameraManager>
) -> Result<Vec<CameraInfo>, String> {
    state.list_cameras()
}

#[command]
pub async fn init_camera(
    device_id: String,
    state: State<'_, MultiCameraManager>
) -> Result<(), String> {
    state.initialize_camera(device_id).await
}

#[command]
pub async fn start_streaming(
    device_id: String,
    state: State<'_, MultiCameraManager>
) -> Result<(), String> {
    let camera = state.get_camera(&device_id)
        .await
        .ok_or_else(|| format!("Camera {} not initialized", device_id))?;

    // Votre logique de streaming
    // ...

    Ok(())
}
```

---

## Features CrabCamera Disponibles

### Core (toujours disponibles)

- `PlatformCamera` - Accès caméra cross-platform
- `CameraSystem` - Gestion de système
- Types de base (`CameraFrame`, `CameraFormat`, etc.)
- Optimisations platform-specific

### Feature "audio"

```rust
use crabcamera::audio::{AudioCapture, AudioDevice};

let device = AudioDevice::default()?;
let capture = AudioCapture::new(device)?;
```

### Feature "recording"

```rust
use crabcamera::recording::{VideoRecorder, RecordingConfig};

let config = RecordingConfig {
    output_path: "video.mp4".into(),
    codec: "H264".to_string(),
    bitrate: 5_000_000,
};

let recorder = VideoRecorder::new(config)?;
```

### Feature "webrtc"

```rust
use crabcamera::webrtc::{WebRtcPeer, SdpOffer};

let peer = WebRtcPeer::new()?;
let offer = peer.create_offer().await?;
```

---

## Exemple Complet : Plugin de Surveillance

```rust
// surveillance-plugin/src/lib.rs

use crabcamera::{
    PlatformCamera, CameraInitParams, CameraFrame, CameraError,
    platform::optimizations,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tauri::{command, plugin::TauriPlugin, Runtime};

pub struct SurveillanceState {
    cameras: Arc<RwLock<Vec<Arc<PlatformCamera>>>>,
    recording: Arc<RwLock<bool>>,
}

impl SurveillanceState {
    pub fn new() -> Self {
        Self {
            cameras: Arc::new(RwLock::new(Vec::new())),
            recording: Arc::new(RwLock::new(false)),
        }
    }
}

#[command]
async fn start_surveillance(
    camera_ids: Vec<String>,
    state: tauri::State<'_, SurveillanceState>,
) -> Result<String, String> {
    let mut cameras = state.cameras.write().await;

    // Initialiser toutes les caméras
    for device_id in camera_ids {
        let params = CameraInitParams::new(device_id.clone())
            .with_format(optimizations::get_optimal_settings().format.unwrap());

        let camera = PlatformCamera::new(params)
            .map_err(|e| format!("Failed to init {}: {}", device_id, e))?;

        // Démarrer le streaming avec détection de mouvement
        camera.start_streaming(Box::new(move |frame: CameraFrame| {
            // Votre logique de détection de mouvement
            if detect_motion(&frame) {
                log::warn!("Motion detected on camera {}", device_id);
                // Sauvegarder la frame, envoyer alerte, etc.
            }
        })).await.map_err(|e| format!("Failed to stream: {}", e))?;

        cameras.push(Arc::new(camera));
    }

    *state.recording.write().await = true;

    Ok(format!("Surveillance started with {} cameras", cameras.len()))
}

fn detect_motion(frame: &CameraFrame) -> bool {
    // Votre algorithme de détection
    false
}

#[command]
async fn stop_surveillance(
    state: tauri::State<'_, SurveillanceState>,
) -> Result<String, String> {
    let mut cameras = state.cameras.write().await;

    for camera in cameras.iter() {
        camera.stop_stream()
            .map_err(|e| format!("Failed to stop: {}", e))?;
    }

    cameras.clear();
    *state.recording.write().await = false;

    Ok("Surveillance stopped".to_string())
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::new("surveillance")
        .invoke_handler(tauri::generate_handler![
            start_surveillance,
            stop_surveillance,
        ])
        .setup(|app, _api| {
            app.manage(SurveillanceState::new());
            Ok(())
        })
        .build()
}
```

---

## Résumé : API CrabCamera Essentielle

### Imports Principaux

```rust
use crabcamera::{
    // Core types
    PlatformCamera,        // La caméra elle-même
    CameraInitParams,      // Configuration d'init
    CameraFrame,           // Frame capturée
    CameraFormat,          // Format vidéo
    CameraError,           // Erreurs

    // Système
    CameraSystem,          // Liste devices, etc.
    CameraDeviceInfo,      // Info sur device

    // Platform utils
    Platform,              // Détection plateforme
    platform::optimizations, // Paramètres optimaux
};
```

### Fonctions Clés

```rust
// Créer une caméra
let camera = PlatformCamera::new(params)?;

// Obtenir le format
let format = camera.get_format();

// Capturer une frame
let frame = camera.capture_frame().await?;

// Streamer
camera.start_streaming(callback).await?;

// Arrêter
camera.stop_stream()?;

// Paramètres optimaux
let params = optimizations::get_optimal_settings();
let photo_format = optimizations::get_photography_format();
```

---

**Voilà !** Vous avez maintenant tout ce qu'il faut pour utiliser CrabCamera comme dépendance dans votre plugin. Les types et fonctions sont réexportés proprement et prêts à l'emploi.
