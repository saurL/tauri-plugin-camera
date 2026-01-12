# FrameSender - Système de gestion intelligente des frames

## Problème résolu

Quand on envoie trop de frames depuis Rust vers le frontend JavaScript, ça crée une accumulation d'objets `FrameEvent` avec de gros `Uint8Array` en mémoire. Même si JavaScript "drop" les frames, le garbage collector ne peut pas suivre le rythme.

Le **FrameSender** résout ce problème en permettant de **vérifier si quelqu'un est prêt à recevoir** avant de traiter/convertir/envoyer une frame.

## Architecture

```
┌─────────────────┐
│  Camera Thread  │
│   (Rust)        │
└────────┬────────┘
         │
         │ Frame arrive
         ▼
    ┌────────────────┐
    │  FrameSender   │ ◄─── can_send() ?
    │  (Rust)        │
    └────────┬───────┘
             │
        ┌────┴────┐
        │         │
   ┌────▼───┐ ┌──▼─────┐
   │Listener│ │Listener│
   │   #1   │ │   #2   │
   │(ready?)│ │(ready?)│
   └────┬───┘ └──┬─────┘
        │        │
        ▼        ▼
    WebRTC   Frontend UI
```

## Utilisation de base

### 1. Créer un FrameSender

```rust
use tauri_plugin_camera::frame_sender::FrameSender;

let sender = FrameSender::new();
```

### 2. Créer des listeners

Chaque "consommateur" de frames (WebRTC connection, UI component, etc.) crée son propre listener :

```rust
// Un listener pour le frontend UI
let ui_listener = sender.create_listener();

// Un listener pour une connexion WebRTC
let webrtc_listener = sender.create_listener();
```

### 3. Dans le callback caméra

```rust
let callback = move |frame: crabcamera::CameraFrame| {
    // ✅ VÉRIFIER avant de traiter
    if !sender.can_send() {
        log::debug!("Frame #{} dropped - no ready listeners", frame_id);
        return; // Pas de traitement = pas d'allocation mémoire !
    }

    // Maintenant on peut traiter en toute sécurité
    let rgb_data = convert_nv12_to_rgba(&frame.data, frame.width, frame.height)?;

    // Envoyer aux listeners
    channel.send(FrameEvent { data: rgb_data, ... })?;
};
```

### 4. Signaler quand on est prêt/occupé

#### Côté Frontend (TypeScript)

```typescript
// Au début du traitement d'une frame
listener.set_ready(false)  // Je suis occupé

// À la fin du traitement
listener.set_ready(true)   // Je suis prêt pour la prochaine
```

#### Côté Rust

Si tu traites des frames côté Rust (ex: WebRTC encoding) :

```rust
// Avant de commencer le traitement
ui_listener.set_ready(false);

// Encoder/traiter la frame
encode_and_send_h264(frame).await?;

// Quand c'est fini
ui_listener.set_ready(true);
```

## État du sender

```rust
// Nombre de listeners prêts
let ready = sender.ready_count();

// Total de listeners
let total = sender.listener_count();

// Description lisible
match sender.state_description() {
    "personne n'écoute" => { /* Aucun listener */ },
    "listeners présents mais aucun prêt" => { /* Tous occupés */ },
    "prêt à envoyer" => { /* Au moins un prêt */ },
}
```

## Exemple complet : Intégration avec desktop.rs

```rust
use crate::frame_sender::FrameSender;

struct ActiveStream {
    camera_id: String,
    sender: Arc<FrameSender>,
    // ... autres champs
}

impl Camera<R> {
    pub async fn start_streaming(
        &self,
        device_id: String,
        channel: Channel<FrameEvent>,
    ) -> Result<String> {
        // Créer le sender
        let sender = Arc::new(FrameSender::new());

        // Créer un listener pour ce channel
        let listener = sender.create_listener();
        listener.set_ready(true); // Prêt au départ

        let sender_clone = sender.clone();
        let callback = move |frame: CameraFrame| {
            // ✅ Vérifier avant de traiter
            if !sender_clone.can_send() {
                return; // Drop sans traitement
            }

            // Traiter et envoyer
            let rgb_data = nv12_to_rgba(&frame.data, frame.width, frame.height)?;
            channel.send(FrameEvent {
                data: rgb_data,
                width: frame.width,
                height: frame.height,
            })?;
        };

        set_callback(device_id.clone(), callback).await?;

        // Stocker le sender et le listener
        let stream = ActiveStream {
            camera_id: device_id,
            sender,
            _listener: listener,
        };

        Ok(session_id)
    }
}
```

## Intégration Frontend (TypeScript)

Pour signaler au backend quand on est prêt/occupé, il faudrait ajouter une commande Tauri :

### Commande Rust

```rust
#[tauri::command]
async fn set_listener_ready(
    session_id: String,
    ready: bool,
    app: AppHandle<R>,
) -> Result<()> {
    let camera = app.camera();
    let stream = camera.get_stream(&session_id).await?;
    stream.listener.set_ready(ready);
    Ok(())
}
```

### Frontend

```typescript
channel.onmessage = async (frame) => {
  // Dire au backend qu'on est occupé
  await invoke('plugin:camera|set_listener_ready', {
    sessionId,
    ready: false
  })

  // Traiter la frame
  await processFrame(frame)

  // Dire au backend qu'on est prêt
  await invoke('plugin:camera|set_listener_ready', {
    sessionId,
    ready: true
  })
}
```

## Avantages

1. **Pas d'accumulation mémoire** - On ne traite que si quelqu'un écoute
2. **Thread-safe** - Utilise des atomics, pas de locks
3. **Multi-listeners** - Support plusieurs consommateurs (WebRTC + UI)
4. **Automatic cleanup** - Quand un listener est drop, il se désenregistre
5. **Zero-cost quand inutilisé** - Juste quelques atomics

## Performance

- `can_send()`: O(1) - juste lire un atomic
- `set_ready()`: O(1) - atomic swap + increment/decrement
- Pas de locks, pas d'allocations
- Thread-safe sans contention

## Tests

Le module inclut 4 tests unitaires :
- `test_frame_sender_no_listeners` - Comportement sans listeners
- `test_frame_sender_with_listener` - Un seul listener
- `test_multiple_listeners` - Plusieurs listeners
- `test_listener_drop` - Cleanup automatique

Lancer les tests :
```bash
cargo test frame_sender
```
