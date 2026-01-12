use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use crabcamera::permissions::PermissionStatus;
pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;
mod utils;
mod webrtc;
use commands::*;
pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Camera;
#[cfg(mobile)]
use mobile::Camera;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the camera APIs.
pub trait CameraExt<R: Runtime> {
    fn camera(&self) -> &Camera<R>;
}

impl<R: Runtime, T: Manager<R>> crate::CameraExt<R> for T {
    fn camera(&self) -> &Camera<R> {
        self.state::<Camera<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("camera")
        .invoke_handler(tauri::generate_handler![
            request_camera_permission,
            get_available_cameras,
            initialize,
            create_offer,
            create_answer,
            set_remote_description,
            add_ice_candidate,
            close_connection,
            get_connection_state,
            start_camera_webrtc_session,
            start_streaming,
            stop_streaming
        ])
        .setup(|app, api| {
            #[cfg(mobile)]
            let camera = mobile::init(app, api)?;
            #[cfg(desktop)]
            let camera = desktop::init(app, api)?;
            app.manage(camera);
            Ok(())
        })
        .build()
}
