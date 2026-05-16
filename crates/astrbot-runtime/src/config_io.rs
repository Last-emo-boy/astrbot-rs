use std::fs;
use std::path::Path;

use astrbot_core::{AstrbotError, Result};

use crate::RuntimeConfig;
use crate::config::migration::config_needs_default_merge;

pub(crate) fn read_runtime_config(path: &Path) -> Result<RuntimeConfig> {
    if !path.exists() {
        let config = RuntimeConfig::default();
        write_runtime_config(path, &config)?;
        return Ok(config);
    }

    let content = fs::read_to_string(path)
        .map_err(|err| AstrbotError::Pipeline(format!("read runtime config: {err}")))?;
    let config: RuntimeConfig = serde_json::from_str(&content)
        .map_err(|err| AstrbotError::Pipeline(format!("parse runtime config: {err}")))?;
    if config_needs_default_merge(&content)? {
        write_runtime_config(path, &config)?;
    }
    Ok(config)
}

pub(crate) fn write_runtime_config(path: &Path, config: &RuntimeConfig) -> Result<()> {
    let serialized = serde_json::to_string_pretty(config)
        .map_err(|err| AstrbotError::Pipeline(format!("serialize runtime config: {err}")))?;
    fs::write(path, serialized)
        .map_err(|err| AstrbotError::Pipeline(format!("write runtime config: {err}")))
}
