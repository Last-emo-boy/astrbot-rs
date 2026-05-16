use astrbot_core::AstrbotError;

use crate::capability::ProviderAdapterMetadata;

pub(super) fn duplicate_provider_type(provider_type: &str) -> AstrbotError {
    AstrbotError::Provider(format!(
        "provider type {provider_type} is already registered"
    ))
}

pub(super) fn missing_factory_error(
    provider_type: &str,
    requested_provider_label: &str,
    unregistered_provider_label: &str,
    metadata: Option<&ProviderAdapterMetadata>,
) -> AstrbotError {
    match metadata {
        Some(metadata) => AstrbotError::Provider(format!(
            "provider type {} has capability {} and cannot be used as {}",
            provider_type, metadata.capability, requested_provider_label
        )),
        None => AstrbotError::Provider(format!(
            "{unregistered_provider_label} {provider_type} is not registered"
        )),
    }
}
