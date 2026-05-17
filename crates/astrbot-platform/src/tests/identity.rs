use std::sync::Arc;

use astrbot_core::{MessageChain, MessageSession, PlatformMemberRole};

use crate::{
    IdentityPermissionResolver, PlatformGroupIdentityInput, PlatformIdentityNormalizer,
    PlatformPermission, PlatformPermissionResolver, RecordingSink, platform_member,
};

#[test]
fn identity_normalizer_builds_sender_and_group_metadata() {
    let identity = PlatformIdentityNormalizer::normalize_identity(
        " user-1 ",
        Some(" Alice ".to_string()),
        Some(
            PlatformGroupIdentityInput::new(" group-1 ")
                .with_name(" Operators ")
                .with_owner_id("owner-1")
                .with_admin_id(" user-1 ")
                .with_admin_id("user-1")
                .with_member(platform_member(
                    "owner-1",
                    Some("Owner".to_string()),
                    PlatformMemberRole::Owner,
                )),
        ),
    );

    let group = identity.group.as_ref().expect("group metadata");
    assert_eq!(identity.sender.user_id, "user-1");
    assert_eq!(identity.sender.display_name.as_deref(), Some("Alice"));
    assert_eq!(group.group_id, "group-1");
    assert_eq!(group.name.as_deref(), Some("Operators"));
    assert_eq!(group.admin_ids, vec!["user-1".to_string()]);
    assert_eq!(identity.effective_role(), PlatformMemberRole::Admin);
}

#[test]
fn identity_permission_resolver_uses_event_identity_role() {
    let sink = Arc::new(RecordingSink::default());
    let session = MessageSession::group("onebot", "group:group-1");
    let identity = PlatformIdentityNormalizer::normalize_identity(
        "owner-1",
        None,
        Some(PlatformGroupIdentityInput::new("group-1").with_owner_id("owner-1")),
    );
    let event = astrbot_core::MessageEvent::new(
        "event-1",
        "onebot",
        "OneBot",
        session,
        astrbot_core::MessageSender::new("owner-1", None),
        MessageChain::plain("hello"),
        sink,
    )
    .with_identity(identity);

    let resolver = IdentityPermissionResolver;
    assert_eq!(
        resolver.resolve_permission(&event),
        PlatformPermission::Owner
    );
    assert!(resolver.allows(&event, PlatformPermission::Admin));
}
