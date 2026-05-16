use crate::{
    CONSOLE_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, ONEBOT_PLATFORM_TYPE, PlatformRegistry,
    WEBCHAT_PLATFORM_TYPE,
};
use astrbot_core::AstrbotError;

#[test]
fn builtins_register_mock_platform_type() {
    let registry = PlatformRegistry::with_builtin_platforms();

    assert!(registry.has_platform(MOCK_PLATFORM_TYPE));
    assert!(registry.has_platform(CONSOLE_PLATFORM_TYPE));
    assert!(registry.has_platform(WEBCHAT_PLATFORM_TYPE));
    assert!(registry.has_platform(ONEBOT_PLATFORM_TYPE));
}

#[test]
fn duplicate_platform_type_is_rejected() {
    let mut registry = PlatformRegistry::new();
    registry
        .register_platform("mock", |_config, _ctx| {
            Err(AstrbotError::Platform("unused".to_string()))
        })
        .expect("first registration should work");

    let duplicate = registry.register_platform("mock", |_config, _ctx| {
        Err(AstrbotError::Platform("unused".to_string()))
    });

    assert!(duplicate.is_err());
}
