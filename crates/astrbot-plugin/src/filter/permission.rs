use astrbot_core::MessageEvent;

use super::EventFilter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionLevel {
    Member,
    Admin,
    Owner,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionScope {
    admin_user_ids: Vec<String>,
    owner_user_ids: Vec<String>,
}

impl PermissionScope {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_admin_user_id(mut self, user_id: impl Into<String>) -> Self {
        push_unique_normalized(&mut self.admin_user_ids, user_id);
        self
    }

    pub fn with_owner_user_id(mut self, user_id: impl Into<String>) -> Self {
        push_unique_normalized(&mut self.owner_user_ids, user_id);
        self
    }

    pub fn is_admin(&self, user_id: &str) -> bool {
        self.owner_user_ids.iter().any(|known| known == user_id)
            || self.admin_user_ids.iter().any(|known| known == user_id)
    }

    pub fn is_owner(&self, user_id: &str) -> bool {
        self.owner_user_ids.iter().any(|known| known == user_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionFilter {
    required: PermissionLevel,
    scope: PermissionScope,
}

impl PermissionFilter {
    pub fn new(required: PermissionLevel, scope: PermissionScope) -> Self {
        Self { required, scope }
    }

    pub fn admin(scope: PermissionScope) -> Self {
        Self::new(PermissionLevel::Admin, scope)
    }

    pub fn owner(scope: PermissionScope) -> Self {
        Self::new(PermissionLevel::Owner, scope)
    }
}

impl EventFilter for PermissionFilter {
    fn matches(&self, event: &MessageEvent) -> bool {
        match self.required {
            PermissionLevel::Member => true,
            PermissionLevel::Admin => self.scope.is_admin(&event.sender.id),
            PermissionLevel::Owner => self.scope.is_owner(&event.sender.id),
        }
    }
}

fn push_unique_normalized(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into().trim().to_string();
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}
