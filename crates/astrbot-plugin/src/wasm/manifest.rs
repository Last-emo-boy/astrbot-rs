//! `plugin.toml` manifest for WASM plugins.
//!
//! The manifest is the single source of truth for everything the host needs
//! to know about a plugin **before** loading it: id, version, the ABI it was
//! compiled against, and the capabilities it requests. We parse it with a
//! tiny hand-rolled TOML reader because the workspace has no `toml` crate and
//! the schema is intentionally flat.

use std::collections::HashMap;

use crate::wasm::abi::ABI_VERSION_MAJOR;
use crate::wasm::capability::{Capability, CapabilitySet};

/// Parsed `plugin.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasmPluginManifest {
    pub id: String,
    pub version: String,
    pub description: Option<String>,
    /// ABI major version the plugin was built against.
    pub abi_major: i32,
    /// ABI minor version the plugin was built against.
    pub abi_minor: i32,
    /// Relative path to the wasm artefact within the plugin directory.
    pub entry: String,
    /// Capabilities the plugin requests from the host.
    pub capabilities: CapabilitySet,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("invalid value for {field}: {message}")]
    InvalidValue {
        field: &'static str,
        message: String,
    },
    #[error("plugin declares ABI {declared}, host implements {host}")]
    AbiMismatch { declared: i32, host: i32 },
    #[error("syntax error on line {line}: {message}")]
    Syntax { line: usize, message: String },
}

impl From<ManifestError> for astrbot_core::AstrbotError {
    fn from(value: ManifestError) -> Self {
        astrbot_core::AstrbotError::Pipeline(value.to_string())
    }
}

impl WasmPluginManifest {
    /// Parse a `plugin.toml` from its textual form.
    pub fn from_toml(source: &str) -> Result<Self, ManifestError> {
        let table = parse_flat_toml(source)?;

        let id = take_required(&table, "id")?;
        let version = take_required(&table, "version")?;
        let entry = table
            .get("entry")
            .cloned()
            .unwrap_or_else(|| "plugin.wasm".to_string());
        let description = table.get("description").cloned();
        let abi_major = parse_int(&table, "abi_major")?
            .or(parse_int(&table, "abi_version")?)
            .ok_or(ManifestError::MissingField("abi_major"))?;
        let abi_minor = parse_int(&table, "abi_minor")?.unwrap_or(0);

        let capabilities = match table.get("capabilities") {
            Some(raw) => {
                parse_capability_list(raw).map_err(|message| ManifestError::InvalidValue {
                    field: "capabilities",
                    message,
                })?
            }
            None => CapabilitySet::empty(),
        };

        Ok(Self {
            id,
            version,
            description,
            abi_major: abi_major as i32,
            abi_minor: abi_minor as i32,
            entry,
            capabilities,
        })
    }

    /// Validate that the plugin's declared ABI is compatible with this host.
    pub fn validate_abi(&self) -> Result<(), ManifestError> {
        if self.abi_major != ABI_VERSION_MAJOR {
            return Err(ManifestError::AbiMismatch {
                declared: self.abi_major,
                host: ABI_VERSION_MAJOR,
            });
        }
        Ok(())
    }

    /// Quick accessor — used both by the loader and by management surfaces.
    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }
}

fn take_required(
    table: &HashMap<String, String>,
    key: &'static str,
) -> Result<String, ManifestError> {
    table
        .get(key)
        .cloned()
        .ok_or(ManifestError::MissingField(key))
}

fn parse_int(
    table: &HashMap<String, String>,
    key: &'static str,
) -> Result<Option<i64>, ManifestError> {
    match table.get(key) {
        None => Ok(None),
        Some(raw) => raw
            .parse::<i64>()
            .map(Some)
            .map_err(|err| ManifestError::InvalidValue {
                field: key,
                message: err.to_string(),
            }),
    }
}

fn parse_capability_list(raw: &str) -> Result<CapabilitySet, String> {
    let trimmed = raw.trim();
    let inner = trimmed
        .strip_prefix('[')
        .and_then(|v| v.strip_suffix(']'))
        .ok_or_else(|| format!("expected TOML array, got `{trimmed}`"))?;

    let mut items: Vec<Capability> = Vec::new();
    for chunk in inner.split(',') {
        let cleaned = chunk.trim().trim_matches('"').trim_matches('\'');
        if cleaned.is_empty() {
            continue;
        }
        items.push(
            cleaned
                .parse::<Capability>()
                .map_err(|err| err.to_string())?,
        );
    }
    Ok(CapabilitySet::from_iter(items))
}

/// Parse a flat `key = value` TOML document. Sections, inline tables, and
/// nested arrays are intentionally unsupported — the schema is meant to stay
/// flat to keep parsing trivial.
fn parse_flat_toml(source: &str) -> Result<HashMap<String, String>, ManifestError> {
    let mut out = HashMap::new();
    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or(ManifestError::Syntax {
            line: line_no,
            message: format!("expected `key = value`, got `{line}`"),
        })?;
        let key = key.trim().to_string();
        let value = strip_inline_comment(value.trim()).trim().to_string();
        let stripped = strip_string_quotes(&value);
        out.insert(key, stripped);
    }
    Ok(out)
}

fn strip_inline_comment(value: &str) -> &str {
    // Honour `#` only outside of quoted strings.
    let bytes = value.as_bytes();
    let mut in_string = false;
    for (idx, byte) in bytes.iter().copied().enumerate() {
        match byte {
            b'"' | b'\'' => in_string = !in_string,
            b'#' if !in_string => return &value[..idx],
            _ => {}
        }
    }
    value
}

fn strip_string_quotes(value: &str) -> String {
    let v = value.trim();
    if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_manifest_parses() {
        let manifest = WasmPluginManifest::from_toml(
            r#"id = "echo"
version = "0.1.0"
abi_major = 1
entry = "echo.wasm"
"#,
        )
        .unwrap();
        assert_eq!(manifest.id, "echo");
        assert_eq!(manifest.version, "0.1.0");
        assert_eq!(manifest.abi_major, 1);
        assert_eq!(manifest.abi_minor, 0);
        assert_eq!(manifest.entry, "echo.wasm");
        assert!(manifest.capabilities.is_empty());
    }

    #[test]
    fn capabilities_array_parses() {
        let manifest = WasmPluginManifest::from_toml(
            r#"id = "echo"
version = "0.1.0"
abi_major = 1
capabilities = ["log", "messaging"]
"#,
        )
        .unwrap();
        assert!(manifest.capabilities.contains(Capability::Log));
        assert!(manifest.capabilities.contains(Capability::Messaging));
        assert!(!manifest.capabilities.contains(Capability::HttpFetch));
    }

    #[test]
    fn missing_required_field_errors() {
        let err =
            WasmPluginManifest::from_toml("version = \"0.1.0\"\nabi_major = 1\n").unwrap_err();
        assert!(matches!(err, ManifestError::MissingField("id")));
    }

    #[test]
    fn validate_abi_rejects_mismatch() {
        let manifest = WasmPluginManifest::from_toml(
            r#"id = "echo"
version = "0.1.0"
abi_major = 99
"#,
        )
        .unwrap();
        assert!(matches!(
            manifest.validate_abi(),
            Err(ManifestError::AbiMismatch { .. })
        ));
    }

    #[test]
    fn validate_abi_accepts_current_host() {
        let toml = format!(
            r#"id = "echo"
version = "0.1.0"
abi_major = {ABI_VERSION_MAJOR}
"#
        );
        let manifest = WasmPluginManifest::from_toml(&toml).unwrap();
        manifest.validate_abi().unwrap();
    }

    #[test]
    fn unknown_capability_rejected() {
        let err = WasmPluginManifest::from_toml(
            r#"id = "echo"
version = "0.1.0"
abi_major = 1
capabilities = ["log", "foo"]
"#,
        )
        .unwrap_err();
        matches!(err, ManifestError::InvalidValue { .. });
    }

    #[test]
    fn inline_comment_is_stripped() {
        let manifest = WasmPluginManifest::from_toml(
            r#"id = "echo" # a comment
version = "0.1.0"
abi_major = 1
"#,
        )
        .unwrap();
        assert_eq!(manifest.id, "echo");
    }
}
