use std::error::Error;
use std::path::PathBuf;

use astrbot_runtime::RuntimeConfig;

pub(super) async fn init(config_path: PathBuf) -> Result<(), Box<dyn Error>> {
    let _ = RuntimeConfig::from_json_file(&config_path)?;
    println!("initialized {}", config_path.display());
    Ok(())
}
