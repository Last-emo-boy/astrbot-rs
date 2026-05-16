use astrbot_core::{AstrbotError, Result};
use serde_json::Value;

use crate::RuntimeConfig;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeConfigMigrationPlan {
    pub missing_default_keys: Vec<String>,
}

impl RuntimeConfigMigrationPlan {
    pub fn is_empty(&self) -> bool {
        self.missing_default_keys.is_empty()
    }
}

pub fn runtime_config_migration_plan(content: &str) -> Result<RuntimeConfigMigrationPlan> {
    let default_value = serde_json::to_value(RuntimeConfig::default()).map_err(|err| {
        AstrbotError::Pipeline(format!("serialize default runtime config: {err}"))
    })?;
    let current_value: Value = serde_json::from_str(content)
        .map_err(|err| AstrbotError::Pipeline(format!("parse runtime config json: {err}")))?;

    let mut missing_default_keys = Vec::new();
    collect_missing_default_keys(
        "",
        &default_value,
        &current_value,
        &mut missing_default_keys,
    );
    Ok(RuntimeConfigMigrationPlan {
        missing_default_keys,
    })
}

pub(crate) fn config_needs_default_merge(content: &str) -> Result<bool> {
    Ok(!runtime_config_migration_plan(content)?.is_empty())
}

fn collect_missing_default_keys(
    path: &str,
    default: &Value,
    current: &Value,
    missing: &mut Vec<String>,
) {
    if let (Value::Object(default_map), Value::Object(current_map)) = (default, current) {
        for (key, default_value) in default_map {
            let next_path = if path.is_empty() {
                key.to_string()
            } else {
                format!("{path}.{key}")
            };

            match current_map.get(key) {
                Some(current_value) => {
                    collect_missing_default_keys(&next_path, default_value, current_value, missing);
                }
                None => missing.push(next_path),
            }
        }
    }
}
