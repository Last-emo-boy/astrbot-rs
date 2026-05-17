use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::rule::{SessionBatchScope, filter_umos_by_scope};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionGroup {
    pub id: String,
    pub name: String,
    pub umos: Vec<String>,
}

impl SessionGroup {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Option<Self> {
        let id = normalize(id);
        let name = normalize(name);
        (!id.is_empty() && !name.is_empty()).then_some(Self {
            id,
            name,
            umos: Vec::new(),
        })
    }

    pub fn with_umos<I, S>(mut self, umos: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.umos = normalized_unique(umos);
        self
    }

    pub fn add_umos<I, S>(&mut self, umos: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.umos = normalized_unique(
            self.umos
                .iter()
                .cloned()
                .chain(umos.into_iter().map(Into::into)),
        );
    }

    pub fn remove_umos<I, S>(&mut self, umos: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let removed = normalized_unique(umos).into_iter().collect::<BTreeSet<_>>();
        self.umos.retain(|umo| !removed.contains(umo));
    }

    pub fn umo_count(&self) -> usize {
        self.umos.len()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionGroupPatch {
    pub name: Option<String>,
    pub umos: Option<Vec<String>>,
    pub add_umos: Vec<String>,
    pub remove_umos: Vec<String>,
}

impl SessionGroupPatch {
    pub fn apply_to(self, group: &mut SessionGroup) {
        if let Some(name) = self.name.and_then(non_empty) {
            group.name = name;
        }

        if let Some(umos) = self.umos {
            group.umos = normalized_unique(umos);
        } else {
            group.add_umos(self.add_umos);
            group.remove_umos(self.remove_umos);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionBatchTarget {
    pub scope: SessionBatchScope,
    pub resolved_umos: Vec<String>,
}

impl SessionBatchTarget {
    pub fn resolve(
        scope: SessionBatchScope,
        all_umos: impl IntoIterator<Item = impl Into<String>>,
        groups: &[SessionGroup],
    ) -> Self {
        let resolved_umos = match &scope {
            SessionBatchScope::CustomGroup(group_id) => groups
                .iter()
                .find(|group| group.id == *group_id)
                .map(|group| group.umos.clone())
                .unwrap_or_default(),
            _ => filter_umos_by_scope(&scope, all_umos),
        };

        Self {
            scope,
            resolved_umos: normalized_unique(resolved_umos),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.resolved_umos.is_empty()
    }
}

fn normalized_unique<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut set = BTreeSet::new();
    for value in values {
        let value = normalize(value);
        if !value.is_empty() {
            set.insert(value);
        }
    }
    set.into_iter().collect()
}

fn non_empty(value: impl Into<String>) -> Option<String> {
    let value = normalize(value);
    (!value.is_empty()).then_some(value)
}

fn normalize(value: impl Into<String>) -> String {
    value.into().trim().to_string()
}
