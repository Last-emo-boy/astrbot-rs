use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use astrbot_session::{
    ProviderCapability, SessionBatchScope, SessionBatchTarget, SessionGroup,
    SessionProviderPreference, SessionRule, SessionRuleKey, SessionRuleSet, SessionRuleValue,
    SessionServiceRule, SessionServiceRulePatch,
};
use async_trait::async_trait;

use crate::SqliteJsonStore;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionBatchUpdateReport {
    pub success_count: usize,
    pub failed_umos: Vec<String>,
}

impl SessionBatchUpdateReport {
    pub fn new(success_count: usize, failed_umos: Vec<String>) -> Self {
        Self {
            success_count,
            failed_umos,
        }
    }

    pub fn failed_count(&self) -> usize {
        self.failed_umos.len()
    }
}

#[async_trait]
pub trait SessionRuleRepository: Send + Sync {
    async fn upsert_rule(&self, rule: SessionRule) -> Result<()>;

    async fn rule_set(&self, umo: &str) -> Result<Option<SessionRuleSet>>;

    async fn list_rule_sets(&self) -> Result<Vec<SessionRuleSet>>;

    async fn delete_rule(&self, umo: &str, key: SessionRuleKey) -> Result<bool>;

    async fn delete_rule_set(&self, umo: &str) -> Result<bool>;

    async fn apply_service_rule_patch(
        &self,
        umos: &[String],
        patch: SessionServiceRulePatch,
    ) -> Result<SessionBatchUpdateReport>;

    async fn set_provider_preference(
        &self,
        umo: &str,
        preference: SessionProviderPreference,
    ) -> Result<()>;

    async fn provider_preference(
        &self,
        umo: &str,
        capability: ProviderCapability,
    ) -> Result<Option<String>>;
}

#[async_trait]
pub trait SessionGroupRepository: Send + Sync {
    async fn upsert_group(&self, group: SessionGroup) -> Result<()>;

    async fn group(&self, group_id: &str) -> Result<Option<SessionGroup>>;

    async fn list_groups(&self) -> Result<Vec<SessionGroup>>;

    async fn delete_group(&self, group_id: &str) -> Result<bool>;

    async fn resolve_batch_target(
        &self,
        scope: SessionBatchScope,
        all_umos: Vec<String>,
    ) -> Result<SessionBatchTarget>;
}

#[derive(Default)]
pub struct InMemorySessionRuleRepository {
    rules: RwLock<HashMap<String, SessionRuleSet>>,
    groups: RwLock<HashMap<String, SessionGroup>>,
}

#[derive(Clone, Debug)]
pub struct SqliteSessionRuleRepository {
    store: SqliteJsonStore,
}

impl SqliteSessionRuleRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

impl InMemorySessionRuleRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SessionRuleRepository for InMemorySessionRuleRepository {
    async fn upsert_rule(&self, rule: SessionRule) -> Result<()> {
        let mut rules = self.rules.write().map_err(rule_lock_error)?;
        let entry = rules
            .entry(rule.umo.clone())
            .or_insert_with(|| SessionRuleSet::new(rule.umo.clone()).expect("rule umo is valid"));
        let updated = entry.clone().with_rule(rule);
        if updated.has_any_rule() {
            *entry = updated;
        }
        Ok(())
    }

    async fn rule_set(&self, umo: &str) -> Result<Option<SessionRuleSet>> {
        let umo = required(umo, "umo")?;
        Ok(self
            .rules
            .read()
            .map_err(rule_lock_error)?
            .get(umo)
            .cloned())
    }

    async fn list_rule_sets(&self) -> Result<Vec<SessionRuleSet>> {
        let mut rule_sets = self
            .rules
            .read()
            .map_err(rule_lock_error)?
            .values()
            .filter(|rule_set| rule_set.has_any_rule())
            .cloned()
            .collect::<Vec<_>>();
        rule_sets.sort_by(|left, right| left.umo.cmp(&right.umo));
        Ok(rule_sets)
    }

    async fn delete_rule(&self, umo: &str, key: SessionRuleKey) -> Result<bool> {
        let umo = required(umo, "umo")?;
        let mut rules = self.rules.write().map_err(rule_lock_error)?;
        let Some(rule_set) = rules.get_mut(umo) else {
            return Ok(false);
        };

        let removed = match key {
            SessionRuleKey::Service => rule_set.service.take().is_some(),
            SessionRuleKey::Plugin => rule_set.plugin.take().is_some(),
            SessionRuleKey::KnowledgeBase => rule_set.knowledge_base.take().is_some(),
            SessionRuleKey::Provider(capability) => {
                let before = rule_set.provider_preferences.len();
                rule_set
                    .provider_preferences
                    .retain(|preference| preference.capability != capability);
                before != rule_set.provider_preferences.len()
            }
        };

        if !rule_set.has_any_rule() {
            rules.remove(umo);
        }
        Ok(removed)
    }

    async fn delete_rule_set(&self, umo: &str) -> Result<bool> {
        let umo = required(umo, "umo")?;
        Ok(self
            .rules
            .write()
            .map_err(rule_lock_error)?
            .remove(umo)
            .is_some())
    }

    async fn apply_service_rule_patch(
        &self,
        umos: &[String],
        patch: SessionServiceRulePatch,
    ) -> Result<SessionBatchUpdateReport> {
        if !patch.has_changes() {
            return Ok(SessionBatchUpdateReport::default());
        }

        let mut rules = self.rules.write().map_err(rule_lock_error)?;
        let mut success_count = 0;
        let mut failed_umos = Vec::new();
        for umo in umos {
            let umo = umo.trim();
            if umo.is_empty() {
                failed_umos.push(umo.to_string());
                continue;
            }
            let rule_set = rules
                .entry(umo.to_string())
                .or_insert_with(|| SessionRuleSet::new(umo).expect("trimmed umo is valid"));
            let mut service = rule_set
                .service
                .take()
                .unwrap_or_else(SessionServiceRule::new);
            service.merge_patch(patch.clone());
            rule_set.service = Some(service);
            success_count += 1;
        }

        Ok(SessionBatchUpdateReport::new(success_count, failed_umos))
    }

    async fn set_provider_preference(
        &self,
        umo: &str,
        preference: SessionProviderPreference,
    ) -> Result<()> {
        let umo = required(umo, "umo")?.to_string();
        self.upsert_rule(
            SessionRule::new(
                umo,
                SessionRuleKey::Provider(preference.capability),
                SessionRuleValue::Provider(preference),
            )
            .expect("umo was validated"),
        )
        .await
    }

    async fn provider_preference(
        &self,
        umo: &str,
        capability: ProviderCapability,
    ) -> Result<Option<String>> {
        let umo = required(umo, "umo")?;
        Ok(self
            .rules
            .read()
            .map_err(rule_lock_error)?
            .get(umo)
            .and_then(|rule_set| rule_set.provider_for(capability).map(ToString::to_string)))
    }
}

#[async_trait]
impl SessionRuleRepository for SqliteSessionRuleRepository {
    async fn upsert_rule(&self, rule: SessionRule) -> Result<()> {
        let mut rule_set = self
            .store
            .get_json::<SessionRuleSet>("session_rule_sets", &rule.umo)?
            .unwrap_or_else(|| SessionRuleSet::new(rule.umo.clone()).expect("rule umo is valid"));
        rule_set = rule_set.with_rule(rule);
        if rule_set.has_any_rule() {
            self.store
                .put_json("session_rule_sets", &rule_set.umo, &rule_set)?;
        }
        Ok(())
    }

    async fn rule_set(&self, umo: &str) -> Result<Option<SessionRuleSet>> {
        let umo = required(umo, "umo")?;
        self.store.get_json("session_rule_sets", umo)
    }

    async fn list_rule_sets(&self) -> Result<Vec<SessionRuleSet>> {
        let mut rule_sets = self
            .store
            .list_json::<SessionRuleSet>("session_rule_sets")?
            .into_iter()
            .filter(SessionRuleSet::has_any_rule)
            .collect::<Vec<_>>();
        rule_sets.sort_by(|left, right| left.umo.cmp(&right.umo));
        Ok(rule_sets)
    }

    async fn delete_rule(&self, umo: &str, key: SessionRuleKey) -> Result<bool> {
        let umo = required(umo, "umo")?;
        let Some(mut rule_set) = self.rule_set(umo).await? else {
            return Ok(false);
        };
        let removed = match key {
            SessionRuleKey::Service => rule_set.service.take().is_some(),
            SessionRuleKey::Plugin => rule_set.plugin.take().is_some(),
            SessionRuleKey::KnowledgeBase => rule_set.knowledge_base.take().is_some(),
            SessionRuleKey::Provider(capability) => {
                let before = rule_set.provider_preferences.len();
                rule_set
                    .provider_preferences
                    .retain(|preference| preference.capability != capability);
                before != rule_set.provider_preferences.len()
            }
        };
        if !removed {
            return Ok(false);
        }
        if rule_set.has_any_rule() {
            self.store.put_json("session_rule_sets", umo, &rule_set)?;
        } else {
            self.store.delete_json("session_rule_sets", umo)?;
        }
        Ok(true)
    }

    async fn delete_rule_set(&self, umo: &str) -> Result<bool> {
        let umo = required(umo, "umo")?;
        self.store.delete_json("session_rule_sets", umo)
    }

    async fn apply_service_rule_patch(
        &self,
        umos: &[String],
        patch: SessionServiceRulePatch,
    ) -> Result<SessionBatchUpdateReport> {
        if !patch.has_changes() {
            return Ok(SessionBatchUpdateReport::default());
        }

        let mut success_count = 0;
        let mut failed_umos = Vec::new();
        for umo in umos {
            let umo = umo.trim();
            if umo.is_empty() {
                failed_umos.push(umo.to_string());
                continue;
            }
            let mut rule_set = self
                .store
                .get_json::<SessionRuleSet>("session_rule_sets", umo)?
                .unwrap_or_else(|| SessionRuleSet::new(umo).expect("trimmed umo is valid"));
            let mut service = rule_set
                .service
                .take()
                .unwrap_or_else(SessionServiceRule::new);
            service.merge_patch(patch.clone());
            rule_set.service = Some(service);
            self.store.put_json("session_rule_sets", umo, &rule_set)?;
            success_count += 1;
        }

        Ok(SessionBatchUpdateReport::new(success_count, failed_umos))
    }

    async fn set_provider_preference(
        &self,
        umo: &str,
        preference: SessionProviderPreference,
    ) -> Result<()> {
        let umo = required(umo, "umo")?.to_string();
        self.upsert_rule(
            SessionRule::new(
                umo,
                SessionRuleKey::Provider(preference.capability),
                SessionRuleValue::Provider(preference),
            )
            .expect("umo was validated"),
        )
        .await
    }

    async fn provider_preference(
        &self,
        umo: &str,
        capability: ProviderCapability,
    ) -> Result<Option<String>> {
        let umo = required(umo, "umo")?;
        Ok(self
            .rule_set(umo)
            .await?
            .and_then(|rule_set| rule_set.provider_for(capability).map(ToString::to_string)))
    }
}

#[async_trait]
impl SessionGroupRepository for InMemorySessionRuleRepository {
    async fn upsert_group(&self, group: SessionGroup) -> Result<()> {
        required(&group.id, "group_id")?;
        required(&group.name, "group_name")?;
        self.groups
            .write()
            .map_err(group_lock_error)?
            .insert(group.id.clone(), group);
        Ok(())
    }

    async fn group(&self, group_id: &str) -> Result<Option<SessionGroup>> {
        let group_id = required(group_id, "group_id")?;
        Ok(self
            .groups
            .read()
            .map_err(group_lock_error)?
            .get(group_id)
            .cloned())
    }

    async fn list_groups(&self) -> Result<Vec<SessionGroup>> {
        let mut groups = self
            .groups
            .read()
            .map_err(group_lock_error)?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        groups.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(groups)
    }

    async fn delete_group(&self, group_id: &str) -> Result<bool> {
        let group_id = required(group_id, "group_id")?;
        Ok(self
            .groups
            .write()
            .map_err(group_lock_error)?
            .remove(group_id)
            .is_some())
    }

    async fn resolve_batch_target(
        &self,
        scope: SessionBatchScope,
        all_umos: Vec<String>,
    ) -> Result<SessionBatchTarget> {
        let groups = self.list_groups().await?;
        Ok(SessionBatchTarget::resolve(scope, all_umos, &groups))
    }
}

#[async_trait]
impl SessionGroupRepository for SqliteSessionRuleRepository {
    async fn upsert_group(&self, group: SessionGroup) -> Result<()> {
        required(&group.id, "group_id")?;
        required(&group.name, "group_name")?;
        self.store.put_json("session_groups", &group.id, &group)
    }

    async fn group(&self, group_id: &str) -> Result<Option<SessionGroup>> {
        let group_id = required(group_id, "group_id")?;
        self.store.get_json("session_groups", group_id)
    }

    async fn list_groups(&self) -> Result<Vec<SessionGroup>> {
        let mut groups = self.store.list_json::<SessionGroup>("session_groups")?;
        groups.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        Ok(groups)
    }

    async fn delete_group(&self, group_id: &str) -> Result<bool> {
        let group_id = required(group_id, "group_id")?;
        self.store.delete_json("session_groups", group_id)
    }

    async fn resolve_batch_target(
        &self,
        scope: SessionBatchScope,
        all_umos: Vec<String>,
    ) -> Result<SessionBatchTarget> {
        let groups = self.list_groups().await?;
        Ok(SessionBatchTarget::resolve(scope, all_umos, &groups))
    }
}

fn required<'a>(value: &'a str, field: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AstrbotError::Pipeline(format!(
            "session rule {field} is required"
        )));
    }
    Ok(value)
}

fn rule_lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("session rule lock: {err}"))
}

fn group_lock_error<T>(err: std::sync::PoisonError<T>) -> AstrbotError {
    AstrbotError::Pipeline(format!("session group lock: {err}"))
}

#[cfg(test)]
mod tests {
    use astrbot_session::{
        ProviderCapability, SessionBatchScope, SessionGroup, SessionProviderPreference,
        SessionRule, SessionRuleKey, SessionRuleValue, SessionServiceRule, SessionServiceRulePatch,
    };

    use super::{
        InMemorySessionRuleRepository, SessionGroupRepository, SessionRuleRepository,
        SqliteSessionRuleRepository,
    };
    use crate::SqliteJsonStore;

    #[tokio::test]
    async fn session_rule_repository_stores_typed_rules_by_umo() {
        let repository = InMemorySessionRuleRepository::new();
        repository
            .upsert_rule(
                SessionRule::new(
                    "webchat:group:room-1",
                    SessionRuleKey::Service,
                    SessionRuleValue::Service(
                        SessionServiceRule::new()
                            .with_session_enabled(true)
                            .with_llm_enabled(false),
                    ),
                )
                .expect("service rule"),
            )
            .await
            .expect("rule should store");
        repository
            .set_provider_preference(
                "webchat:group:room-1",
                SessionProviderPreference::new(ProviderCapability::ChatCompletion, "provider-a")
                    .expect("provider preference"),
            )
            .await
            .expect("provider should store");

        let rule_set = repository
            .rule_set("webchat:group:room-1")
            .await
            .expect("rule should load")
            .expect("rule set");

        assert_eq!(
            rule_set.service.as_ref().and_then(|rule| rule.llm_enabled),
            Some(false)
        );
        assert_eq!(
            repository
                .provider_preference("webchat:group:room-1", ProviderCapability::ChatCompletion)
                .await
                .expect("provider should load")
                .as_deref(),
            Some("provider-a")
        );
    }

    #[tokio::test]
    async fn session_rule_repository_applies_service_patch_to_batch_targets() {
        let repository = InMemorySessionRuleRepository::new();
        let report = repository
            .apply_service_rule_patch(
                &[
                    "webchat:group:room-1".to_string(),
                    "webchat:private:user-1".to_string(),
                ],
                SessionServiceRulePatch {
                    session_enabled: Some(false),
                    llm_enabled: Some(false),
                    tts_enabled: None,
                },
            )
            .await
            .expect("patch should apply");

        assert_eq!(report.success_count, 2);
        assert_eq!(report.failed_count(), 0);
        assert_eq!(
            repository
                .rule_set("webchat:private:user-1")
                .await
                .expect("rule set should load")
                .expect("rule set")
                .service
                .expect("service")
                .session_enabled,
            Some(false)
        );
    }

    #[tokio::test]
    async fn session_group_repository_resolves_custom_group_targets() {
        let repository = InMemorySessionRuleRepository::new();
        repository
            .upsert_group(
                SessionGroup::new("team", "Team")
                    .expect("group")
                    .with_umos(["webchat:group:room-1", "webchat:private:user-1"]),
            )
            .await
            .expect("group should store");

        let target = repository
            .resolve_batch_target(
                SessionBatchScope::CustomGroup("team".to_string()),
                vec!["ignored".to_string()],
            )
            .await
            .expect("target should resolve");

        assert_eq!(target.resolved_umos.len(), 2);
    }

    #[tokio::test]
    async fn sqlite_session_rule_repository_persists_rules_and_groups() {
        let store = SqliteJsonStore::open_in_memory().expect("sqlite store should open");
        let repository = SqliteSessionRuleRepository::new(store.clone());
        repository
            .upsert_rule(
                SessionRule::new(
                    "webchat:group:room-1",
                    SessionRuleKey::Service,
                    SessionRuleValue::Service(
                        SessionServiceRule::new()
                            .with_session_enabled(true)
                            .with_llm_enabled(false),
                    ),
                )
                .expect("service rule"),
            )
            .await
            .expect("rule should store");
        repository
            .upsert_group(
                SessionGroup::new("team", "Team")
                    .expect("group")
                    .with_umos(["webchat:group:room-1"]),
            )
            .await
            .expect("group should store");

        let reloaded = SqliteSessionRuleRepository::new(store);
        assert_eq!(
            reloaded
                .rule_set("webchat:group:room-1")
                .await
                .expect("rule should load")
                .expect("rule set")
                .service
                .expect("service")
                .llm_enabled,
            Some(false)
        );
        assert_eq!(
            reloaded
                .group("team")
                .await
                .expect("group should load")
                .expect("group")
                .umo_count(),
            1
        );
    }
}
