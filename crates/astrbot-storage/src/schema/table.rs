#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageColumnType {
    Text,
    Integer,
    Boolean,
    Json,
    Binary,
    Timestamp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageColumn {
    pub name: String,
    pub column_type: StorageColumnType,
    pub nullable: bool,
    pub primary_key: bool,
    pub unique: bool,
    pub default_value: Option<String>,
}

impl StorageColumn {
    pub fn new(name: impl Into<String>, column_type: StorageColumnType) -> Self {
        Self {
            name: name.into(),
            column_type,
            nullable: false,
            primary_key: false,
            unique: false,
            default_value: None,
        }
    }

    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }

    pub fn primary_key(mut self) -> Self {
        self.primary_key = true;
        self
    }

    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    pub fn default_value(mut self, default_value: impl Into<String>) -> Self {
        self.default_value = Some(default_value.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageTable {
    pub name: String,
    pub columns: Vec<StorageColumn>,
    pub unique_keys: Vec<Vec<String>>,
}

impl StorageTable {
    pub fn new(name: impl Into<String>, columns: Vec<StorageColumn>) -> Self {
        Self {
            name: name.into(),
            columns,
            unique_keys: Vec::new(),
        }
    }

    pub fn with_unique_key<I, S>(mut self, columns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.unique_keys
            .push(columns.into_iter().map(Into::into).collect());
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StorageSchema {
    pub name: String,
    pub version: u32,
    pub tables: Vec<StorageTable>,
}

impl StorageSchema {
    pub fn new(name: impl Into<String>, version: u32, tables: Vec<StorageTable>) -> Self {
        Self {
            name: name.into(),
            version,
            tables,
        }
    }

    pub fn table(&self, name: &str) -> Option<&StorageTable> {
        self.tables.iter().find(|table| table.name == name)
    }
}
