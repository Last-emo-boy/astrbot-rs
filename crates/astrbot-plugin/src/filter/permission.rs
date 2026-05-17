use astrbot_core::MessageEvent;

use super::EventFilter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionLevel {
    Member,
    Admin,
    Owner,
}

impl PermissionLevel {
    pub fn allows(self, required: Self) -> bool {
        self.rank() >= required.rank()
    }

    pub fn max(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Member => 0,
            Self::Admin => 1,
            Self::Owner => 2,
        }
    }
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
        self.permission_for_user(user_id)
            .allows(PermissionLevel::Admin)
    }

    pub fn is_owner(&self, user_id: &str) -> bool {
        self.permission_for_user(user_id)
            .allows(PermissionLevel::Owner)
    }

    pub fn permission_for_user(&self, user_id: &str) -> PermissionLevel {
        if self.owner_user_ids.iter().any(|known| known == user_id) {
            PermissionLevel::Owner
        } else if self.admin_user_ids.iter().any(|known| known == user_id) {
            PermissionLevel::Admin
        } else {
            PermissionLevel::Member
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PermissionResolver;

impl PermissionResolver {
    pub fn resolve(&self, event: &MessageEvent, scope: &PermissionScope) -> PermissionLevel {
        let identity_permission = event
            .identity()
            .map(|identity| {
                if identity.is_owner() {
                    PermissionLevel::Owner
                } else if identity.is_admin_or_owner() {
                    PermissionLevel::Admin
                } else {
                    PermissionLevel::Member
                }
            })
            .unwrap_or(PermissionLevel::Member);

        identity_permission.max(scope.permission_for_user(&event.sender.id))
    }

    pub fn allows(
        &self,
        event: &MessageEvent,
        scope: &PermissionScope,
        required: PermissionLevel,
    ) -> bool {
        self.resolve(event, scope).allows(required)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PermissionFilter {
    required: PermissionLevel,
    scope: PermissionScope,
    resolver: PermissionResolver,
}

impl PermissionFilter {
    pub fn new(required: PermissionLevel, scope: PermissionScope) -> Self {
        Self {
            required,
            scope,
            resolver: PermissionResolver,
        }
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
        self.resolver.allows(event, &self.scope, self.required)
    }
}

fn push_unique_normalized(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into().trim().to_string();
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}
