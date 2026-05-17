use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformMemberRole {
    #[default]
    Member,
    Admin,
    Owner,
}

impl PlatformMemberRole {
    pub fn is_admin_or_owner(self) -> bool {
        matches!(self, Self::Admin | Self::Owner)
    }

    pub fn is_owner(self) -> bool {
        matches!(self, Self::Owner)
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformMemberProfile {
    pub user_id: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub role: PlatformMemberRole,
}

impl PlatformMemberProfile {
    pub fn new(user_id: impl Into<String>, display_name: Option<String>) -> Self {
        Self {
            user_id: normalize_string(user_id),
            display_name: normalize_optional_string(display_name),
            role: PlatformMemberRole::Member,
        }
    }

    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = normalize_optional_string(Some(display_name.into()));
        self
    }

    pub fn with_role(mut self, role: PlatformMemberRole) -> Self {
        self.role = role;
        self
    }

    pub fn display_name_or_id(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.user_id)
    }

    pub fn is_admin_or_owner(&self) -> bool {
        self.role.is_admin_or_owner()
    }

    pub fn is_owner(&self) -> bool {
        self.role.is_owner()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformGroupMetadata {
    pub group_id: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub owner_id: Option<String>,
    #[serde(default)]
    pub admin_ids: Vec<String>,
    #[serde(default)]
    pub members: Vec<PlatformMemberProfile>,
}

impl PlatformGroupMetadata {
    pub fn new(group_id: impl Into<String>) -> Self {
        Self {
            group_id: normalize_string(group_id),
            name: None,
            avatar_url: None,
            owner_id: None,
            admin_ids: Vec::new(),
            members: Vec::new(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = normalize_optional_string(Some(name.into()));
        self
    }

    pub fn with_avatar_url(mut self, avatar_url: impl Into<String>) -> Self {
        self.avatar_url = normalize_optional_string(Some(avatar_url.into()));
        self
    }

    pub fn with_owner_id(mut self, owner_id: impl Into<String>) -> Self {
        self.owner_id = normalize_optional_string(Some(owner_id.into()));
        self
    }

    pub fn with_admin_id(mut self, admin_id: impl Into<String>) -> Self {
        push_unique_normalized(&mut self.admin_ids, admin_id);
        self
    }

    pub fn with_member(mut self, member: PlatformMemberProfile) -> Self {
        if !member.user_id.is_empty()
            && !self
                .members
                .iter()
                .any(|known| known.user_id == member.user_id)
        {
            self.members.push(member);
        }
        self
    }

    pub fn role_for_user(&self, user_id: &str) -> PlatformMemberRole {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return PlatformMemberRole::Member;
        }

        if self.owner_id.as_deref() == Some(user_id) {
            return PlatformMemberRole::Owner;
        }

        if self.admin_ids.iter().any(|known| known == user_id) {
            return PlatformMemberRole::Admin;
        }

        self.members
            .iter()
            .find(|member| member.user_id == user_id)
            .map(|member| member.role)
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformIdentity {
    pub sender: PlatformMemberProfile,
    pub group: Option<PlatformGroupMetadata>,
}

impl PlatformIdentity {
    pub fn new(sender: PlatformMemberProfile) -> Self {
        Self {
            sender,
            group: None,
        }
    }

    pub fn with_group(mut self, group: PlatformGroupMetadata) -> Self {
        self.group = Some(group);
        self
    }

    pub fn effective_role(&self) -> PlatformMemberRole {
        let group_role = self
            .group
            .as_ref()
            .map(|group| group.role_for_user(&self.sender.user_id))
            .unwrap_or_default();
        self.sender.role.max(group_role)
    }

    pub fn is_admin_or_owner(&self) -> bool {
        self.effective_role().is_admin_or_owner()
    }

    pub fn is_owner(&self) -> bool {
        self.effective_role().is_owner()
    }

    pub fn group_id(&self) -> Option<&str> {
        self.group.as_ref().map(|group| group.group_id.as_str())
    }
}

fn normalize_string(value: impl Into<String>) -> String {
    value.into().trim().to_string()
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn push_unique_normalized(values: &mut Vec<String>, value: impl Into<String>) {
    let value = normalize_string(value);
    if !value.is_empty() && !values.iter().any(|known| known == &value) {
        values.push(value);
    }
}
