//! Per-store resource ceilings enforced through [`wasmtime::ResourceLimiter`].
//!
//! Plugins receive bounded memory, table, instance, and table-element budgets.
//! Requests beyond a configured ceiling are denied at growth time so the guest
//! observes a deterministic trap instead of pushing the host into OOM.

use wasmtime::ResourceLimiter;

/// Static ceilings applied to every plugin store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PluginResourceLimits {
    /// Maximum linear memory in bytes. Defaults to 32 MiB.
    pub max_memory_bytes: usize,
    /// Maximum number of table elements across all tables.
    pub max_table_elements: usize,
    /// Maximum number of concurrent component instances inside the store.
    pub max_instances: usize,
    /// Maximum number of tables that can be allocated.
    pub max_tables: usize,
    /// Maximum number of linear memories that can be allocated.
    pub max_memories: usize,
}

impl Default for PluginResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 32 * 1024 * 1024,
            max_table_elements: 10_000,
            max_instances: 16,
            max_tables: 16,
            max_memories: 16,
        }
    }
}

impl PluginResourceLimits {
    /// Convenience constructor for tests that only care about the memory limit.
    pub fn with_memory(max_memory_bytes: usize) -> Self {
        Self {
            max_memory_bytes,
            ..Self::default()
        }
    }
}

/// `ResourceLimiter` implementation tracking live usage of a plugin store.
pub struct PluginResourceLimiter {
    limits: PluginResourceLimits,
    current_memory_bytes: usize,
}

impl PluginResourceLimiter {
    pub fn new(limits: PluginResourceLimits) -> Self {
        Self {
            limits,
            current_memory_bytes: 0,
        }
    }

    pub fn limits(&self) -> &PluginResourceLimits {
        &self.limits
    }

    pub fn current_memory_bytes(&self) -> usize {
        self.current_memory_bytes
    }
}

impl ResourceLimiter for PluginResourceLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.limits.max_memory_bytes {
            return Ok(false);
        }
        if let Some(maximum) = maximum {
            if desired > maximum {
                return Ok(false);
            }
        }
        self.current_memory_bytes = desired;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.limits.max_table_elements {
            return Ok(false);
        }
        if let Some(maximum) = maximum {
            if desired > maximum {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn instances(&self) -> usize {
        self.limits.max_instances
    }

    fn tables(&self) -> usize {
        self.limits.max_tables
    }

    fn memories(&self) -> usize {
        self.limits.max_memories
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_conservative() {
        let limits = PluginResourceLimits::default();
        assert_eq!(limits.max_memory_bytes, 32 * 1024 * 1024);
        assert!(limits.max_instances <= 32);
    }

    #[test]
    fn memory_growth_denied_above_ceiling() {
        let mut limiter = PluginResourceLimiter::new(PluginResourceLimits::with_memory(64 * 1024));
        let allowed = limiter.memory_growing(0, 128 * 1024, None).unwrap();
        assert!(!allowed);
        assert_eq!(limiter.current_memory_bytes(), 0);
    }

    #[test]
    fn memory_growth_allowed_under_ceiling() {
        let mut limiter = PluginResourceLimiter::new(PluginResourceLimits::with_memory(64 * 1024));
        let allowed = limiter.memory_growing(0, 32 * 1024, None).unwrap();
        assert!(allowed);
        assert_eq!(limiter.current_memory_bytes(), 32 * 1024);
    }

    #[test]
    fn table_growth_denied_above_ceiling() {
        let mut limiter = PluginResourceLimiter::new(PluginResourceLimits {
            max_table_elements: 100,
            ..PluginResourceLimits::default()
        });
        assert!(!limiter.table_growing(0, 200, None).unwrap());
        assert!(limiter.table_growing(0, 50, None).unwrap());
    }
}
