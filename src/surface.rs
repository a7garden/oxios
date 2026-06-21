//! Surface activation helpers.
//!
//! Re-exports the Surface trait from oxios-gateway and provides
//! the activation function used by the binary's `cmd_serve()`.

pub use oxios_gateway::{Surface, SurfaceContext};

use anyhow::Result;
use oxios_gateway::ActiveWebDist;
use std::path::Path;
use std::sync::Arc;

use crate::kernel::Kernel;

/// Build the list of available surfaces.
pub fn build_surfaces() -> Vec<Box<dyn Surface>> {
    #[cfg(feature = "web")]
    let surfaces: Vec<Box<dyn Surface>> = vec![Box::new(crate::api::WebSurface::new())];
    #[cfg(not(feature = "web"))]
    let surfaces: Vec<Box<dyn Surface>> = vec![];
    surfaces
}

/// Activate all enabled surfaces.
///
/// Surfaces receive full kernel access. If a surface also returns a channel,
/// it is registered with the gateway for message routing.
pub async fn activate_surfaces(
    kernel: &Kernel,
    config_path: &Path,
    web_dist: ActiveWebDist,
) -> Result<Vec<tokio::task::JoinHandle<()>>> {
    let surfaces = build_surfaces();
    let config = kernel.config();
    let mut all_tasks = Vec::new();

    // Read surface names from config — surfaces are listed separately from channels.
    let surface_names: Vec<String> = config
        .surfaces
        .as_ref()
        .map(|s| s.enabled.clone())
        .unwrap_or_else(|| {
            // Default: enable web if the feature is compiled in.
            #[cfg(feature = "web")]
            {
                vec!["web".to_string()]
            }
            #[cfg(not(feature = "web"))]
            {
                vec![]
            }
        });

    let surface_map: std::collections::HashMap<&str, &dyn Surface> =
        surfaces.iter().map(|s| (s.name(), s.as_ref())).collect();

    for name in &surface_names {
        match surface_map.get(name.as_str()) {
            Some(surface) => {
                let ctx = SurfaceContext {
                    kernel: kernel.handle(),
                    config: Arc::new(parking_lot::RwLock::new(config.clone())),
                    config_path: config_path.to_path_buf(),
                    web_dist: web_dist.clone(),
                };
                match surface.start(ctx).await {
                    Ok(handle) => {
                        tracing::info!(surface = %name, "Surface activated");
                        if let Some(channel) = handle.channel
                            && let Err(e) = kernel.register_channel(channel).await
                        {
                            tracing::error!(surface = %name, error = %e, "Failed to register surface channel");
                        }
                        all_tasks.extend(handle.tasks);
                    }
                    Err(e) => {
                        tracing::error!(surface = %name, error = %e, "Failed to activate surface")
                    }
                }
            }
            None => tracing::warn!(
                surface = %name,
                "Surface '{}' not available (not compiled in). Available: {}",
                name,
                surface_map.keys().cloned().collect::<Vec<_>>().join(", ")
            ),
        }
    }

    Ok(all_tasks)
}
