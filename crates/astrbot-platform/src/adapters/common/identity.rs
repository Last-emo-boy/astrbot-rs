use astrbot_core::{
    MessageSender, PlatformGroupMetadata, PlatformIdentity, PlatformMemberProfile,
    PlatformMemberRole,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlatformGroupIdentityInput {
    pub group_id: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub owner_id: Option<String>,
    pub admin_ids: Vec<String>,
    pub members: Vec<PlatformMemberProfile>,
}

impl PlatformGroupIdentityInput {
    pub fn new(group_id: impl Into<String>) -> Self {
        Self {
            group_id: group_id.into(),
            name: None,
            avatar_url: None,
            owner_id: None,
            admin_ids: Vec::new(),
            members: Vec::new(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_avatar_url(mut self, avatar_url: impl Into<String>) -> Self {
        self.avatar_url = Some(avatar_url.into());
        self
    }

    pub fn with_owner_id(mut self, owner_id: impl Into<String>) -> Self {
        self.owner_id = Some(owner_id.into());
        self
    }

    pub fn with_admin_id(mut self, admin_id: impl Into<String>) -> Self {
        push_unique_trimmed(&mut self.admin_ids, admin_id);
        self
    }

    pub fn with_member(mut self, member: PlatformMemberProfile) -> Self {
        if !member.user_id.trim().is_empty()
            && !self
                .members
                .iter()
                .any(|known| known.user_id == member.user_id)
        {
            self.members.push(member);
        }
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlatformIdentityNormalizer;

impl PlatformIdentityNormalizer {
    pub fn normalize_sender(
        sender_id: impl Into<String>,
        display_name: Option<String>,
    ) -> PlatformMemberProfile {
        PlatformMemberProfile::new(sender_id, display_name)
    }

    pub fn normalize_group(input: PlatformGroupIdentityInput) -> PlatformGroupMetadata {
        let mut group = PlatformGroupMetadata::new(input.group_id);
        if let Some(name) = input.name {
            group = group.with_name(name);
        }
        if let Some(avatar_url) = input.avatar_url {
            group = group.with_avatar_url(avatar_url);
        }
        if let Some(owner_id) = input.owner_id {
            group = group.with_owner_id(owner_id);
        }
        for admin_id in input.admin_ids {
            group = group.with_admin_id(admin_id);
        }
        for member in input.members {
            group = group.with_member(member);
        }
        group
    }

    pub fn normalize_identity(
        sender_id: impl Into<String>,
        display_name: Option<String>,
        group: Option<PlatformGroupIdentityInput>,
    ) -> PlatformIdentity {
        let sender = Self::normalize_sender(sender_id, display_name);
        let mut identity = PlatformIdentity::new(sender);
        if let Some(group) = group {
            identity = identity.with_group(Self::normalize_group(group));
        }
        identity
    }

    pub fn normalize_direct_event(sender: &MessageSender) -> PlatformIdentity {
        Self::normalize_identity(sender.id.clone(), sender.display_name.clone(), None)
    }
}

pub fn platform_member(
    user_id: impl Into<String>,
    display_name: Option<String>,
    role: PlatformMemberRole,
) -> PlatformMemberProfile {
    PlatformMemberProfile::new(user_id, display_name).with_role(role)
}

fn push_unique_trimmed(values: &mut Vec<String>, value: impl Into<String>) {
    let value = value.into().trim().to_string();
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}
