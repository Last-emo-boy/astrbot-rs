use std::time::Duration;

use astrbot_core::MessageChain;

use crate::{
    ActiveEventInterruption, ActiveEventRegistry, ProviderCapability, SessionBatchScope,
    SessionBatchTarget, SessionGroup, SessionGroupPatch, SessionKnowledgeBaseRule,
    SessionLockManager, SessionPluginRule, SessionProviderPreference, SessionRule, SessionRuleKey,
    SessionRuleSet, SessionRuleValue, SessionServiceRule, SessionServiceRulePatch,
    SessionWaitDecision, SessionWaiter, SessionWaitingEvent, filter_umos_by_scope,
};

#[test]
fn session_waiter_records_history_and_finishes_without_platforms() {
    let mut waiter = SessionWaiter::new();

    assert_eq!(
        waiter.register("webchat:direct:user-1", Duration::from_secs(30), true),
        SessionWaitDecision::Waiting
    );
    assert_eq!(
        waiter.trigger(SessionWaitingEvent::new(
            "webchat:direct:user-1",
            MessageChain::plain("next"),
        )),
        SessionWaitDecision::Triggered
    );

    let history = waiter
        .history("webchat:direct:user-1")
        .expect("history should be available");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].plain_text(), "next");

    let registration = waiter
        .finish("webchat:direct:user-1")
        .expect("wait should finish");
    assert_eq!(registration.history().len(), 1);
    assert!(!waiter.has_wait("webchat:direct:user-1"));
}

#[test]
fn session_waiter_keep_zero_timeout_removes_wait() {
    let mut waiter = SessionWaiter::new();
    waiter.register("session", Duration::from_secs(30), false);

    assert_eq!(
        waiter.keep("session", Duration::ZERO, true),
        SessionWaitDecision::TimedOut
    );
    assert_eq!(
        waiter.trigger(SessionWaitingEvent::new(
            "session",
            MessageChain::plain("late")
        )),
        SessionWaitDecision::Missing
    );
}

#[tokio::test]
async fn session_lock_manager_serializes_per_session_work() {
    let locks = SessionLockManager::new();
    let first = locks.acquire("session-1").await;
    let second_locks = locks.clone();
    let blocked = tokio::spawn(async move {
        let guard = second_locks.acquire("session-1").await;
        guard.release().await;
        "acquired"
    });

    tokio::task::yield_now().await;
    assert!(!blocked.is_finished());
    assert_eq!(locks.tracked_session_count().await, 1);

    first.release().await;
    assert_eq!(blocked.await.expect("task should complete"), "acquired");
    assert_eq!(locks.tracked_session_count().await, 0);
}

#[test]
fn active_event_registry_tracks_stop_and_agent_stop_separately() {
    let mut registry = ActiveEventRegistry::new();
    registry.register("event-1", "session");
    registry.register("event-2", "session");
    registry.register("event-3", "other");

    assert_eq!(
        registry.interrupt_session(
            "session",
            ActiveEventInterruption::RequestAgentStop,
            Some("event-1"),
        ),
        1
    );
    assert!(
        registry
            .record("event-2")
            .expect("event should exist")
            .agent_stop_requested
    );
    assert!(
        !registry
            .record("event-1")
            .expect("event should exist")
            .agent_stop_requested
    );

    assert_eq!(
        registry.interrupt_session("session", ActiveEventInterruption::StopEvent, None),
        2
    );
    assert!(
        registry
            .record("event-1")
            .expect("event should exist")
            .stop_event_requested
    );
    assert_eq!(registry.session_event_count("session"), 2);
    registry.unregister("event-1");
    assert_eq!(registry.session_event_count("session"), 1);
    assert_eq!(registry.session_event_count("other"), 1);
}

#[test]
fn session_rule_set_tracks_service_plugins_kb_and_provider_preferences() {
    let service = SessionServiceRule::new()
        .with_session_enabled(true)
        .with_llm_enabled(false)
        .with_custom_name("Research group")
        .with_persona_id("analyst");
    let plugin = SessionPluginRule::new()
        .with_enabled_plugin("search")
        .with_disabled_plugin("games");
    let kb = SessionKnowledgeBaseRule::new()
        .with_kb_id("kb-1")
        .with_top_k(8);
    let provider = SessionProviderPreference::new(ProviderCapability::ChatCompletion, "deepseek")
        .expect("provider preference");

    let rule_set = SessionRuleSet::new(" webchat:group:room-1 ")
        .expect("rule set")
        .with_rule(
            SessionRule::new(
                "webchat:group:room-1",
                SessionRuleKey::Service,
                SessionRuleValue::Service(service),
            )
            .expect("service rule"),
        )
        .with_rule(
            SessionRule::new(
                "webchat:group:room-1",
                SessionRuleKey::Plugin,
                SessionRuleValue::Plugin(plugin),
            )
            .expect("plugin rule"),
        )
        .with_rule(
            SessionRule::new(
                "webchat:group:room-1",
                SessionRuleKey::KnowledgeBase,
                SessionRuleValue::KnowledgeBase(kb),
            )
            .expect("kb rule"),
        )
        .with_rule(
            SessionRule::new(
                "webchat:group:room-1",
                SessionRuleKey::Provider(ProviderCapability::ChatCompletion),
                SessionRuleValue::Provider(provider),
            )
            .expect("provider rule"),
        );

    assert_eq!(rule_set.umo, "webchat:group:room-1");
    assert_eq!(
        rule_set.service.as_ref().and_then(|rule| rule.llm_enabled),
        Some(false)
    );
    assert!(
        !rule_set
            .plugin
            .as_ref()
            .expect("plugin rule")
            .is_plugin_enabled("games")
    );
    assert_eq!(
        rule_set.knowledge_base.as_ref().expect("kb rule").kb_ids,
        vec!["kb-1".to_string()]
    );
    assert_eq!(
        rule_set.provider_for(ProviderCapability::ChatCompletion),
        Some("deepseek")
    );
    assert_eq!(
        SessionRuleKey::Provider(ProviderCapability::TextToSpeech).storage_key(),
        "provider_perf_text_to_speech"
    );
}

#[test]
fn session_service_patch_and_batch_scope_select_targets_outside_pipeline() {
    let mut rule = SessionServiceRule::new().with_llm_enabled(true);
    rule.merge_patch(SessionServiceRulePatch {
        llm_enabled: Some(false),
        tts_enabled: None,
        session_enabled: Some(true),
    });

    assert_eq!(rule.llm_enabled, Some(false));
    assert_eq!(rule.session_enabled, Some(true));

    let all = [
        "webchat:group:room-1",
        "webchat:private:user-1",
        "onebot:GroupMessage:42",
    ];
    assert_eq!(
        filter_umos_by_scope(&SessionBatchScope::Group, all),
        vec![
            "onebot:GroupMessage:42".to_string(),
            "webchat:group:room-1".to_string()
        ]
    );
}

#[test]
fn session_groups_resolve_custom_batch_targets_without_web_routes() {
    let mut group = SessionGroup::new("team", "Team Sessions")
        .expect("group")
        .with_umos(["webchat:group:room-1", "webchat:private:user-1"]);
    SessionGroupPatch {
        add_umos: vec!["onebot:group:42".to_string()],
        remove_umos: vec!["webchat:private:user-1".to_string()],
        ..SessionGroupPatch::default()
    }
    .apply_to(&mut group);

    assert_eq!(group.umo_count(), 2);
    assert_eq!(
        group.umos,
        vec![
            "onebot:group:42".to_string(),
            "webchat:group:room-1".to_string()
        ]
    );

    let target = SessionBatchTarget::resolve(
        SessionBatchScope::CustomGroup("team".to_string()),
        ["ignored"],
        &[group],
    );

    assert_eq!(target.resolved_umos.len(), 2);
    assert!(!target.is_empty());
}
