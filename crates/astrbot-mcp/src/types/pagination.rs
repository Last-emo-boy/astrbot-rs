use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpCursor(String);

impl McpCursor {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into().trim().to_string();
        (!value.is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpListPage<T> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<McpCursor>,
}

impl<T> McpListPage<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            items,
            next_cursor: None,
        }
    }

    pub fn with_next_cursor(mut self, next_cursor: impl Into<String>) -> Self {
        self.next_cursor = McpCursor::new(next_cursor);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::McpListPage;

    #[test]
    fn list_page_keeps_optional_non_empty_cursor() {
        let page = McpListPage::new(vec![1, 2]).with_next_cursor(" cursor-2 ");

        assert_eq!(page.items, vec![1, 2]);
        assert_eq!(
            page.next_cursor.expect("cursor should be present").as_str(),
            "cursor-2"
        );
        assert!(
            McpListPage::new(vec![1])
                .with_next_cursor(" ")
                .next_cursor
                .is_none()
        );
    }
}
