use crate::{UmopConfigRoute, UmopConfigRoutePattern, UmopConfigRouter};

#[test]
fn umop_route_pattern_preserves_session_colons_and_matches_wildcards() {
    let pattern =
        UmopConfigRoutePattern::parse("webchat:group:room:*").expect("pattern should parse");
    let target =
        UmopConfigRoutePattern::parse("webchat:group:room:alpha").expect("target should parse");

    assert_eq!(pattern.platform_id(), "webchat");
    assert_eq!(pattern.message_type(), "group");
    assert_eq!(pattern.session_id(), "room:*");
    assert!(pattern.matches(&target));
}

#[test]
fn umop_router_resolves_first_matching_config_route() {
    let router = UmopConfigRouter::new(vec![
        UmopConfigRoute::new("webchat::", "webchat-default"),
        UmopConfigRoute::new("webchat:group:room-*", "room-config"),
    ])
    .expect("routes should validate");

    assert_eq!(
        router.resolve_config_id("webchat:group:room-1"),
        Some("webchat-default")
    );
    assert_eq!(router.resolve_config_id("console:group:room-1"), None);
}

#[test]
fn umop_router_updates_and_rejects_invalid_route_patterns() {
    let mut router = UmopConfigRouter::default();

    router
        .set_route("webchat:private:*", "private-config")
        .expect("route should be accepted");
    router
        .set_route("webchat:private:*", "updated-config")
        .expect("route should be replaced");

    assert_eq!(router.routes().len(), 1);
    assert_eq!(
        router.resolve_config_id("webchat:private:user-1"),
        Some("updated-config")
    );
    assert!(
        router
            .delete_route("webchat:private:*")
            .expect("valid route")
    );
    assert!(router.set_route("invalid", "bad").is_err());
}
