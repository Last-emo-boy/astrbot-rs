//! Plugin capability model.
//!
//! Capabilities are the unit of permission granted to a WASM plugin.
//! Anything sensitive that a host function does — logging, sending chat
//! messages, KV access, outbound HTTP — must be declared up front in the
//! plugin's `plugin.toml` and re-granted by the host operator.
//!
//! The default stance is **deny everything**: an empty capability list means
//! the plugin can only run pure computations.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// All capabilities the host knows how to grant. Adding a variant is a minor
/// ABI bump (`ABI_VERSION_MINOR`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Append log lines via `host_log`. Always cheap, but still gated so
    /// plugins can't spam.
    Log,
    /// Buffer outbound messages via `host_send_message`.
    Messaging,
    /// Read/write the persistent KV store. Storage scoped per plugin id.
    Kv,
    /// Make outbound HTTP requests via `host_http_fetch`. The host may layer
    /// allow-lists on top.
    HttpFetch,
    /// Invoke host-registered tools via `host_call_tool`.
    Tools,
}

impl Capability {
    /// Identifier used in `plugin.toml` and host capability checks.
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Log => "log",
            Capability::Messaging => "messaging",
            Capability::Kv => "kv",
            Capability::HttpFetch => "http_fetch",
            Capability::Tools => "tools",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown capability: {0}")]
pub struct UnknownCapability(pub String);

impl FromStr for Capability {
    type Err = UnknownCapability;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value.trim() {
            "log" => Capability::Log,
            "messaging" => Capability::Messaging,
            "kv" => Capability::Kv,
            "http_fetch" => Capability::HttpFetch,
            "tools" => Capability::Tools,
            other => return Err(UnknownCapability(other.to_string())),
        })
    }
}

/// A bundle of granted capabilities. Operations on the set are
/// allocation-cheap because the underlying storage is a sorted small vector
/// — there are only ever a handful of capability variants.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CapabilitySet(Vec<Capability>);

impl CapabilitySet {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn from_iter<I: IntoIterator<Item = Capability>>(iter: I) -> Self {
        let mut items: Vec<Capability> = iter.into_iter().collect();
        items.sort_by_key(|c| c.as_str());
        items.dedup();
        Self(items)
    }

    pub fn from_strs<'a, I: IntoIterator<Item = &'a str>>(
        iter: I,
    ) -> Result<Self, UnknownCapability> {
        let mut out = Vec::new();
        for value in iter {
            out.push(Capability::from_str(value)?);
        }
        Ok(Self::from_iter(out))
    }

    pub fn contains(&self, capability: Capability) -> bool {
        self.0.contains(&capability)
    }

    pub fn iter(&self) -> impl Iterator<Item = Capability> + '_ {
        self.0.iter().copied()
    }

    pub fn as_str_vec(&self) -> Vec<String> {
        self.0.iter().map(|c| c.as_str().to_string()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_string_roundtrip() {
        for variant in [
            Capability::Log,
            Capability::Messaging,
            Capability::Kv,
            Capability::HttpFetch,
            Capability::Tools,
        ] {
            let parsed: Capability = variant.as_str().parse().unwrap();
            assert_eq!(parsed, variant);
        }
    }

    #[test]
    fn unknown_capability_rejected() {
        assert!("does_not_exist".parse::<Capability>().is_err());
    }

    #[test]
    fn set_deduplicates_and_sorts() {
        let set = CapabilitySet::from_iter([
            Capability::Messaging,
            Capability::Log,
            Capability::Log,
        ]);
        assert_eq!(set.len(), 2);
        assert!(set.contains(Capability::Log));
        assert!(set.contains(Capability::Messaging));
        assert!(!set.contains(Capability::Kv));
    }

    #[test]
    fn empty_set_denies_all() {
        let set = CapabilitySet::empty();
        for variant in [
            Capability::Log,
            Capability::Messaging,
            Capability::Kv,
            Capability::HttpFetch,
            Capability::Tools,
        ] {
            assert!(!set.contains(variant));
        }
    }
}
