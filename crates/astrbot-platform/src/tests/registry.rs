use crate::{
    CONSOLE_PLATFORM_TYPE, DINGTALK_PLATFORM_TYPE, DISCORD_PLATFORM_TYPE, KOOK_PLATFORM_TYPE,
    MISSKEY_PLATFORM_TYPE, MOCK_PLATFORM_TYPE, ONEBOT_PLATFORM_TYPE, PlatformRegistry,
    QQ_OFFICIAL_PLATFORM_TYPE, QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE, SATORI_PLATFORM_TYPE,
    WEBCHAT_PLATFORM_TYPE, WECOM_KF_PLATFORM_TYPE, WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE,
};
use astrbot_core::AstrbotError;

#[test]
fn builtins_register_mock_platform_type() {
    let registry = PlatformRegistry::with_builtin_platforms();

    assert!(registry.has_platform(MOCK_PLATFORM_TYPE));
    assert!(registry.has_platform(CONSOLE_PLATFORM_TYPE));
    assert!(registry.has_platform(WEBCHAT_PLATFORM_TYPE));
    assert!(registry.has_platform(ONEBOT_PLATFORM_TYPE));
    assert!(registry.has_platform(DINGTALK_PLATFORM_TYPE));
    assert!(registry.has_platform(DISCORD_PLATFORM_TYPE));
    assert!(registry.has_platform(KOOK_PLATFORM_TYPE));
    assert!(registry.has_platform(MISSKEY_PLATFORM_TYPE));
    assert!(registry.has_platform(SATORI_PLATFORM_TYPE));
    assert!(registry.has_platform(QQ_OFFICIAL_PLATFORM_TYPE));
    assert!(registry.has_platform(QQ_OFFICIAL_WEBHOOK_PLATFORM_TYPE));
    assert!(registry.has_platform(WECOM_KF_PLATFORM_TYPE));
    assert!(registry.has_platform(WEIXIN_OFFICIAL_ACCOUNT_PLATFORM_TYPE));
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
