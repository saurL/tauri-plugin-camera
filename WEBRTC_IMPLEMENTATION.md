# Guide d'Implémentation WebRTC avec Streaming Caméra

## Vue d'ensemble

Ce guide explique comment mettre en place un système de streaming vidéo via WebRTC utilisant :

- **Backend Tauri** : capture caméra + encodage H.264 en temps réel
- **Frontend WebRTC** : affichage du flux vidéo en direct
- **Signalisation** : échange SDP et candidats ICE pour établir la connexion P2P

---

## Architecture Générale

### Flux de Données

```
Caméra (YUV420)
    ↓
connect_camera_to_webrtc() [encode YUV→H.264 en boucle]
    ↓
push_h264_sample() [envoie chaque frame H.264]
    ↓
WebRTC Track [transport P2P via ICE/DTLS]
    ↓
Frontend [décode H.264 avec WebCodecs/ffmpeg]
    ↓
Écran
```

### Composants Clés

- **WebRTCManager** : gère les peer connections, les tracks H.264
- **Camera** : gère le streaming caméra, l'encodage H.264
- **Commands WebRTC** : expose les commandes Tauri (create_offer, set_remote_description, add_ice_candidate, etc.)

---

## Flow d'Initialisation

### 1. Démarrer la Session Complète (Backend)

**Commande :** `start_camera_webrtc_session(device_id, ice_servers)`

**Ce qu'elle fait :**

1. Initialise le système caméra
2. Génère un `connection_id` unique (UUID)
3. Crée une PeerConnection avec les ICE servers
4. Attache un track H.264 vide
5. Lance le streaming caméra (`start_streaming`)
6. Lie la caméra au track WebRTC via `connect_camera_to_webrtc`
7. Génère une **offre SDP**
8. Retourne : `(sdp_offer, connection_id)`

**Code d'appel côté Frontend :**

```javascript
const response = await invoke("start_camera_webrtc_session", {
  deviceId: "camera-0",
  iceServers: [{ urls: ["stun:stun.l.google.com:19302"] }],
});
const { sdp_type, sdp, connection_id } = response;
```

**Résultat :** L'offre SDP et l'ID de la connexion pour les étapes suivantes.

---

## Flow de Signalisation WebRTC

### 2. Envoyer l'Offre à l'Autre Pair

Après avoir reçu l'offre du backend :

```javascript
// Frontend reçoit (sdp_offer, connection_id) du backend
const peerConnection = new RTCPeerConnection({ iceServers: ... });
await peerConnection.setRemoteDescription(
  new RTCSessionDescription({ type: 'offer', sdp: sdp_offer })
);
```

### 3. Créer une Réponse (Answerer)

Si tu es l'answerer (récepteur du flux) :

```javascript
const answer = await peerConnection.createAnswer();
await peerConnection.setLocalDescription(answer);

// Envoyer l'answer au backend via la commande Tauri
await invoke("set_remote_description", {
  connectionId,
  description: {
    type: "answer",
    sdp: answer.sdp,
  },
});
```

**Commande Backend :** `set_remote_description(connection_id, { type: 'answer', sdp })`

### 4. Échanger les Candidats ICE

Les candidats ICE arrivent **après** l'offre, au fil du temps. Chaque pair doit envoyer ses candidats à l'autre.

#### Côté Frontend (envoyer candidats au backend) :

```javascript
peerConnection.onicecandidate = (event) => {
  if (event.candidate) {
    invoke("add_ice_candidate", {
      connectionId,
      candidate: {
        candidate: event.candidate.candidate,
        sdpMid: event.candidate.sdpMid,
        sdpMlineIndex: event.candidate.sdpMlineIndex,
      },
    });
  }
};
```

#### Côté Backend (via la commande Tauri) :

**Commande :** `add_ice_candidate(connection_id, { candidate, sdpMid, sdpMlineIndex })`

Cette commande ajoute le candidat ICE à la PeerConnection backend.

---

## À Quoi Sert `add_ice_candidate` ?

### Problème : NAT/Firewall

Les machines ne sont pas directement accessibles sur Internet. Elles sont derrière :

- **NAT** (Network Address Translation)
- **Firewall** (bloque les connexions entrantes)

### Solution : ICE (Interactive Connectivity Establishment)

ICE découvre **progressivement** les chemins possibles pour se connecter :

1. **Host Candidate** : adresse locale (ex: 192.168.1.100:12345)

   - Rapide mais ne fonctionne que si les deux pairs sont sur le même réseau

2. **STUN Candidate** : adresse publique découverte via un serveur STUN

   - Fonctionne si pas de NAT symétrique

3. **TURN Candidate** : relai qui forward le trafic
   - Fonctionne dans tous les cas (mais plus coûteux)

### Timeline

```
T=0ms : Création de la PeerConnection
        → Offre SDP créée (candidats host si connus)

T=50ms : UADetected host candidate
        → onicecandidate() → envoyer au pair via add_ice_candidate

T=200ms : STUN réussit
         → onicecandidate() avec nouveau candidat STUN

T=300ms : TURN accesible
         → onicecandidate() avec candidat TURN

T=500ms : ICE connecté 🎉
         → connectionState = 'connected'
         → Vidéo commence à arriver
```

**Sans `add_ice_candidate` :** tu relies que le premier candidat (host). Si NAT, ça échoue.  
**Avec `add_ice_candidate` :** tu essaies tous les chemins → meilleure chance de connexion.

---

## Fermer la Connexion Proprement

### Commande Backend

**Commande :** `close_connection(connection_id)`

**Ce qu'elle fait :**

1. Ferme la PeerConnection WebRTC
2. **Logs** le device_id associé (pour que tu saches quel streaming arrêter)
3. Nettoie la mapping `connection_id → device_id`

**Code Frontend :**

```javascript
await invoke("close_connection", { connectionId });
```

**Important :** `close_connection` ferme **la connexion WebRTC**, pas le streaming caméra.

### Arrêter le Streaming Caméra Séparément

Il faut aussi arrêter la capture caméra côté backend. À appeler **après** `close_connection` :

```javascript
// Exemple : si tu as un endpoint pour stop_streaming
await invoke("stop_streaming", { sessionId });
```

**Note :** Actuellement, le backend **log** quel device_id était associé à la connexion fermée, mais l'appel à `stop_streaming` reste à charge du frontend. Une future optimisation pourrait automatiser cela dans `close_connection`.

---

## Exemple Complet : Frontend en JavaScript

```javascript
let peerConnection;
let connectionId;

// 1. Initialiser la session complète
async function startWebRTCStream(deviceId) {
  const response = await invoke("start_camera_webrtc_session", {
    deviceId,
    iceServers: [{ urls: ["stun:stun.l.google.com:19302"] }],
  });

  const [sdpData, connId] = response;
  connectionId = connId;

  console.log("Offre générée, connection_id:", connectionId);
  console.log("SDP Offre:", sdpData.sdp);

  // TODO: Envoyer sdpData.sdp à l'autre pair (websocket, API, etc.)
  return { sdp: sdpData.sdp, connectionId };
}

// 2. Recevoir la réponse et établir la connexion
async function setRemoteAnswer(answerSdp) {
  await invoke("set_remote_description", {
    connectionId,
    description: {
      type: "answer",
      sdp: answerSdp,
    },
  });
  console.log("Réponse configurée");
}

// 3. Écouter et envoyer les candidats ICE
async function setupICEHandling(peerConnection) {
  peerConnection.onicecandidate = async (event) => {
    if (event.candidate) {
      console.log("Nouveau candidat ICE:", event.candidate);

      await invoke("add_ice_candidate", {
        connectionId,
        candidate: {
          candidate: event.candidate.candidate,
          sdpMid: event.candidate.sdpMid,
          sdpMlineIndex: event.candidate.sdpMlineIndex,
        },
      });
    } else {
      console.log("Gathering complété");
    }
  };

  peerConnection.onconnectionstatechange = () => {
    console.log("État connexion:", peerConnection.connectionState);
    if (peerConnection.connectionState === "connected") {
      console.log("🎉 WebRTC connecté! Vidéo devrait arriver...");
    }
  };
}

// 4. Arrêter la session
async function stopWebRTCStream() {
  if (connectionId) {
    await invoke("close_connection", { connectionId });
    console.log("Connexion fermée");

    // TODO: aussi arrêter le streaming caméra si applicable
    // await invoke('stop_streaming', { sessionId });
  }
}
```

---

## Checklist d'Implémentation

- [ ] Backend : `start_camera_webrtc_session` appelé avec device_id valide
- [ ] Frontend : reçoit offre SDP et connection_id
- [ ] Frontend : envoie l'offre au pair distant (websocket)
- [ ] Pair distant : crée answer et envoie SDP réponse
- [ ] Frontend : appelle `set_remote_description` avec l'answer
- [ ] Frontend : écoute `onicecandidate` et appelle `add_ice_candidate` pour chaque candidat
- [ ] Pair distant : ajoute aussi les candidats ICE du frontend
- [ ] WebRTC : connectionState passe à 'connected'
- [ ] Vidéo H.264 commence à arriver sur le WebRTC track
- [ ] Frontend : décode le flux H.264 (WebCodecs API ou ffmpeg)
- [ ] Cleanup : `close_connection` appelé avant de détruire la session

---

## Débogage

### Logs Utiles

```rust
// Dans le backend, cherche les logs :
log::info!("WebRTC encoding task started for connection: {}", connection_id);
log::error!("Failed to push H.264 sample: {}", e);
log::info!("Closed connection {}, associated device {} ...", id, dev_id);
```

### Points de Blocage Courants

1. **Vidéo ne démarre pas**

   - Vérifier que `connect_camera_to_webrtc` encode bien en I420 (log les frames)
   - Vérifier que WebRTC est `connected` (pas juste `connecting`)

2. **ICE ne remonte pas**

   - Vérifier que STUN/TURN sont accessibles
   - S'assurer que `add_ice_candidate` est appelé côté backend

3. **Offre/Réponse invalide**
   - Vérifier que SDP n'est pas vide
   - Vérifier le type (offer/answer) avant `setRemoteDescription`

---

## Prochaines Améliorations

- [ ] Fermeture auto du streaming caméra dans `close_connection`
- [ ] Retry automatique sur ICE failure
- [ ] Métriques de latence / qualité vidéo
- [ ] Gestion des reconnexions
- [ ] Support multi-caméras simultanées
