use astrbot_core::{
    MessageSessionKind, PlatformGroupMetadata, PlatformIdentity, PlatformMemberProfile,
};

use crate::{
    CommandFilter, EventFilter, MessageSessionKindFilter, PermissionFilter, PermissionScope,
    PlatformFilter, RegexFilter,
};

use super::event;

#[test]
fn command_filter_matches_alias_and_prefix() {
    let filter = CommandFilter::new("/ping").with_alias("p").with_prefix("!");

    assert!(filter.matches(&event("!ping now")));
    assert!(filter.matches(&event("!p now")));
    assert!(!filter.matches(&event("/ping now")));
}

#[test]
fn typed_filters_match_platform_session_permission_and_regex() {
    let mut group = event("hello 123");
    group.session = group.session.with_kind(MessageSessionKind::Group);

    assert!(PlatformFilter::new("console").matches(&group));
    assert!(MessageSessionKindFilter::group().matches(&group));

    let scope = PermissionScope::new().with_admin_user_id("user");
    assert!(PermissionFilter::admin(scope).matches(&group));

    let regex = RegexFilter::new(r"\d+").expect("regex should compile");
    assert!(regex.matches(&group));
}

#[test]
fn permission_filter_uses_event_identity_roles_before_static_scope() {
    let mut group = event("admin command");
    group.session = group.session.with_kind(MessageSessionKind::Group);
    group.set_identity(
        PlatformIdentity::new(PlatformMemberProfile::new("user", None))
            .with_group(PlatformGroupMetadata::new("group").with_admin_id("user")),
    );

    assert!(PermissionFilter::admin(PermissionScope::new()).matches(&group));
    assert!(!PermissionFilter::owner(PermissionScope::new()).matches(&group));
}

#[test]
fn permission_filter_keeps_static_scope_compatibility() {
    let user = event("owner command");
    let scope = PermissionScope::new().with_owner_user_id("user");

    assert!(PermissionFilter::owner(scope).matches(&user));
}
