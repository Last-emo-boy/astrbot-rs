use astrbot_core::{MessageEvent, PlatformMemberRole};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformPermission {
    Member,
    Admin,
    Owner,
}

impl From<PlatformMemberRole> for PlatformPermission {
    fn from(role: PlatformMemberRole) -> Self {
        match role {
            PlatformMemberRole::Member => Self::Member,
            PlatformMemberRole::Admin => Self::Admin,
            PlatformMemberRole::Owner => Self::Owner,
        }
    }
}

pub trait PlatformPermissionResolver: Send + Sync {
    fn resolve_permission(&self, event: &MessageEvent) -> PlatformPermission;

    fn allows(&self, event: &MessageEvent, required: PlatformPermission) -> bool {
        permission_allows(self.resolve_permission(event), required)
    }
}

#[derive(Clone, Debug, Default)]
pub struct IdentityPermissionResolver;

impl PlatformPermissionResolver for IdentityPermissionResolver {
    fn resolve_permission(&self, event: &MessageEvent) -> PlatformPermission {
        event
            .identity()
            .map(|identity| identity.effective_role().into())
            .unwrap_or(PlatformPermission::Member)
    }
}

pub fn permission_allows(actual: PlatformPermission, required: PlatformPermission) -> bool {
    permission_rank(actual) >= permission_rank(required)
}

fn permission_rank(permission: PlatformPermission) -> u8 {
    match permission {
        PlatformPermission::Member => 0,
        PlatformPermission::Admin => 1,
        PlatformPermission::Owner => 2,
    }
}
