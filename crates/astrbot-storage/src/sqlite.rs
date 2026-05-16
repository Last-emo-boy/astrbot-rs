use std::path::PathBuf;

use crate::schema::{StorageColumnType, StorageSchema};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlitePragma {
    pub key: String,
    pub value: String,
}

impl SqlitePragma {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteStorageConfig {
    pub path: PathBuf,
    pub pragmas: Vec<SqlitePragma>,
}

impl SqliteStorageConfig {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            pragmas: Self::astrbot_default_pragmas(),
        }
    }

    pub fn astrbot_default_pragmas() -> Vec<SqlitePragma> {
        vec![
            SqlitePragma::new("journal_mode", "WAL"),
            SqlitePragma::new("synchronous", "NORMAL"),
            SqlitePragma::new("cache_size", "20000"),
            SqlitePragma::new("temp_store", "MEMORY"),
            SqlitePragma::new("mmap_size", "134217728"),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqliteStoragePlan {
    pub config: SqliteStorageConfig,
    pub schema: StorageSchema,
}

impl SqliteStoragePlan {
    pub fn new(config: SqliteStorageConfig, schema: StorageSchema) -> Self {
        Self { config, schema }
    }

    pub fn astrbot_main(path: impl Into<PathBuf>) -> Self {
        Self::new(
            SqliteStorageConfig::new(path),
            StorageSchema::astrbot_main_v4(),
        )
    }

    pub fn create_table_statements(&self) -> Vec<String> {
        self.schema
            .tables
            .iter()
            .map(|table| {
                let mut column_defs = table
                    .columns
                    .iter()
                    .map(|column| {
                        let mut parts = vec![
                            column.name.clone(),
                            sqlite_type(&column.column_type).to_string(),
                        ];
                        if !column.nullable {
                            parts.push("NOT NULL".to_string());
                        }
                        if column.primary_key {
                            parts.push("PRIMARY KEY".to_string());
                        }
                        if column.unique {
                            parts.push("UNIQUE".to_string());
                        }
                        if let Some(default_value) = &column.default_value {
                            parts.push(format!("DEFAULT {default_value}"));
                        }
                        parts.join(" ")
                    })
                    .collect::<Vec<_>>();

                for unique_key in &table.unique_keys {
                    column_defs.push(format!("UNIQUE({})", unique_key.join(", ")));
                }

                format!(
                    "CREATE TABLE IF NOT EXISTS {} ({})",
                    table.name,
                    column_defs.join(", ")
                )
            })
            .collect()
    }
}

fn sqlite_type(column_type: &StorageColumnType) -> &'static str {
    match column_type {
        StorageColumnType::Text => "TEXT",
        StorageColumnType::Integer => "INTEGER",
        StorageColumnType::Boolean => "INTEGER",
        StorageColumnType::Json => "JSON",
        StorageColumnType::Timestamp => "DATETIME",
        StorageColumnType::Binary => "BLOB",
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteStoragePlan;

    #[test]
    fn sqlite_plan_keeps_astrbot_pragmas_and_schema_tables() {
        let plan = SqliteStoragePlan::astrbot_main("data.db");

        assert!(plan.config.pragmas.iter().any(|p| p.key == "journal_mode"));
        assert!(
            plan.create_table_statements()
                .iter()
                .any(|sql| sql.contains("CREATE TABLE IF NOT EXISTS conversations"))
        );
    }
}
