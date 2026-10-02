use std::error::Error;

use tauri::{AppHandle, Manager};

use crate::commands::AppState;

pub fn setup(app: &AppHandle) -> Result<(), Box<dyn Error>> {
    let config_dir = app.path().app_config_dir()?;
    let cache_dir = app.path().app_cache_dir()?;
    std::fs::create_dir_all(&config_dir)?;
    std::fs::create_dir_all(cache_dir.join("analyses"))?;
    std::fs::create_dir_all(cache_dir.join("markers"))?;
    std::fs::create_dir_all(cache_dir.join("proxies"))?;
    app.manage(AppState::new(config_dir, cache_dir));
    Ok(())
}
