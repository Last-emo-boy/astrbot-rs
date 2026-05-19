use std::collections::HashMap;
use std::sync::RwLock;

use astrbot_core::{AstrbotError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::SqliteJsonStore;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    pub key_id: String,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub scopes: Vec<String>,
    pub created_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SqliteApiKeyRepository {
    store: SqliteJsonStore,
}

impl SqliteApiKeyRepository {
    pub fn new(store: SqliteJsonStore) -> Self {
        Self { store }
    }
}

impl ApiKeyRecord {
    pub fn new(
        key_id: impl Into<String>,
        name: impl Into<String>,
        key_hash: impl Into<String>,
        key_prefix: impl Into<String>,
        scopes: impl IntoIterator<Item = impl Into<String>>,
        created_by: impl Into<String>,
    ) -> Self {
        Self {
            key_id: key_id.into(),
            name: name.into(),
            key_hash: key_hash.into(),
            key_prefix: key_prefix.into(),
            scopes: scopes.into_iter().map(Into::into).collect(),
            created_by: created_by.into(),
            last_used_at: None,
            expires_at: None,
            revoked_at: None,
        }
    }

    pub fn with_last_used_at(mut self, last_used_at: impl Into<String>) -> Self {
        self.last_used_at = Some(last_used_at.into());
        self
    }

    pub fn with_expires_at(mut self, expires_at: impl Into<String>) -> Self {
        self.expires_at = Some(expires_at.into());
        self
    }

    pub fn revoked(mut self, revoked_at: impl Into<String>) -> Self {
        self.revoked_at = Some(revoked_at.into());
        self
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked_at.is_some()
    }

    pub fn is_expired_at(&self, now_unix: u64) -> bool {
        let Some(expires_at) = self.expires_at.as_deref() else {
            return false;
        };
        parse_timestamp(expires_at).is_some_and(|expires_at| expires_at <= now_unix)
    }
}

#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    async fn store_api_key(&self, record: ApiKeyRecord) -> Result<()>;

    async fn api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKeyRecord>>;

    async fn list_api_keys(&self) -> Result<Vec<ApiKeyRecord>>;

    async fn revoke_api_key(&self, key_id: &str, revoked_at: String) -> Result<bool>;

    async fn delete_api_key(&self, key_id: &str) -> Result<bool>;
}

fn parse_timestamp(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Some(value) = value.strip_prefix("unix:") {
        return value.parse().ok();
    }
    if let Ok(value) = value.parse() {
        return Some(value);
    }
    parse_rfc3339_timestamp(value).and_then(|timestamp| u64::try_from(timestamp).ok())
}

fn parse_rfc3339_timestamp(value: &str) -> Option<i64> {
    let (date, time) = value.split_once('T')?;
    let mut date_parts = date.split('-');
    let year = date_parts.next()?.parse::<i32>().ok()?;
    let month = date_parts.next()?.parse::<u32>().ok()?;
    let day = date_parts.next()?.parse::<u32>().ok()?;
    if date_parts.next().is_some() {
        return None;
    }

    let (time, offset_seconds) = split_rfc3339_time_offset(time)?;
    let time = time.split_once('.').map_or(time, |(time, _)| time);
    let mut time_parts = time.split(':');
    let hour = time_parts.next()?.parse::<u32>().ok()?;
    let minute = time_parts.next()?.parse::<u32>().ok()?;
    let second = time_parts.next()?.parse::<u32>().ok()?;
    if time_parts.next().is_some() {
        return None;
    }

    unix_timestamp(year, month, day, hour, minute, second)
        .map(|timestamp| timestamp - i64::from(offset_seconds))
}

fn split_rfc3339_time_offset(value: &str) -> Option<(&str, i32)> {
    if let Some(value) = value.strip_suffix('Z') {
        return Some((value, 0));
    }
    let Some(offset_start) = value.rfind(['+', '-']) else {
        return Some((value, 0));
    };
    let (time, offset) = value.split_at(offset_start);
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let offset = &offset[1..];
    let mut parts = offset.split(':');
    let hours = parts.next()?.parse::<i32>().ok()?;
    let minutes = parts.next()?.parse::<i32>().ok()?;
    if parts.next().is_some() || hours > 23 || minutes > 59 {
        return None;
    }
    Some((time, sign * (hours * 3_600 + minutes * 60)))
}

fn unix_timestamp(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let month_days = days_in_month(year, month)?;
    if day > month_days {
        return None;
    }

    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + i64::from(hour) * 3_600 + i64::from(minute) * 60 + i64::from(second))
}

fn days_in_month(year: i32, month: u32) -> Option<u32> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = i32::try_from(month).expect("month should fit i32");
    let doy = (153 * (mp + if mp > 2 { -3 } else { 9 }) + 2) / 5
        + i32::try_from(day).expect("day should fit i32")
        - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    i64::from(era) * 146_097 + i64::from(doe) - 719_468
}

#[derive(Default)]
pub struct InMemoryApiKeyRepository {
    api_keys: RwLock<HashMap<String, ApiKeyRecord>>,
}

impl InMemoryApiKeyRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ApiKeyRepository for InMemoryApiKeyRepository {
    async fn store_api_key(&self, record: ApiKeyRecord) -> Result<()> {
        self.api_keys
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))?
            .insert(record.key_id.clone(), record);
        Ok(())
    }

    async fn api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKeyRecord>> {
        self.api_keys
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))
            .map(|api_keys| {
                api_keys
                    .values()
                    .find(|record| record.key_hash == key_hash)
                    .cloned()
            })
    }

    async fn list_api_keys(&self) -> Result<Vec<ApiKeyRecord>> {
        let mut api_keys = self
            .api_keys
            .read()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))?
            .values()
            .cloned()
            .collect::<Vec<_>>();
        api_keys.sort_by(|left, right| left.key_id.cmp(&right.key_id));
        Ok(api_keys)
    }

    async fn revoke_api_key(&self, key_id: &str, revoked_at: String) -> Result<bool> {
        let mut api_keys = self
            .api_keys
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))?;
        let Some(record) = api_keys.get_mut(key_id) else {
            return Ok(false);
        };
        record.revoked_at = Some(revoked_at);
        Ok(true)
    }

    async fn delete_api_key(&self, key_id: &str) -> Result<bool> {
        Ok(self
            .api_keys
            .write()
            .map_err(|err| AstrbotError::Pipeline(format!("api key lock: {err}")))?
            .remove(key_id)
            .is_some())
    }
}

#[async_trait]
impl ApiKeyRepository for SqliteApiKeyRepository {
    async fn store_api_key(&self, record: ApiKeyRecord) -> Result<()> {
        self.store.put_json("api_keys", &record.key_id, &record)
    }

    async fn api_key_by_hash(&self, key_hash: &str) -> Result<Option<ApiKeyRecord>> {
        Ok(self
            .list_api_keys()
            .await?
            .into_iter()
            .find(|record| record.key_hash == key_hash))
    }

    async fn list_api_keys(&self) -> Result<Vec<ApiKeyRecord>> {
        let mut api_keys = self.store.list_json::<ApiKeyRecord>("api_keys")?;
        api_keys.sort_by(|left, right| left.key_id.cmp(&right.key_id));
        Ok(api_keys)
    }

    async fn revoke_api_key(&self, key_id: &str, revoked_at: String) -> Result<bool> {
        let Some(mut record) = self.store.get_json::<ApiKeyRecord>("api_keys", key_id)? else {
            return Ok(false);
        };
        record.revoked_at = Some(revoked_at);
        self.store.put_json("api_keys", &record.key_id, &record)?;
        Ok(true)
    }

    async fn delete_api_key(&self, key_id: &str) -> Result<bool> {
        self.store.delete_json("api_keys", key_id)
    }
}

#[cfg(test)]
mod tests {
    use super::{ApiKeyRecord, ApiKeyRepository, InMemoryApiKeyRepository, SqliteApiKeyRepository};
    use crate::SqliteJsonStore;

    #[test]
    fn api_key_record_expiry_accepts_unix_and_source_datetime_shapes() {
        assert!(
            ApiKeyRecord::new("key-1", "Expired", "hash", "ak_", ["chat"], "admin")
                .with_expires_at("unix:1")
                .is_expired_at(2)
        );
        assert!(
            ApiKeyRecord::new("key-2", "Expired", "hash", "ak_", ["chat"], "admin")
                .with_expires_at("1970-01-01T00:00:01Z")
                .is_expired_at(2)
        );
        assert!(
            ApiKeyRecord::new("key-3", "Expired", "hash", "ak_", ["chat"], "admin")
                .with_expires_at("1970-01-01T08:00:01+08:00")
                .is_expired_at(2)
        );
        assert!(
            !ApiKeyRecord::new("key-4", "Active", "hash", "ak_", ["chat"], "admin")
                .with_expires_at("2999-01-01T00:00:00Z")
                .is_expired_at(2)
        );
    }

    #[tokio::test]
    async fn api_key_repository_stores_finds_and_revokes_keys() {
        let repository = InMemoryApiKeyRepository::new();
        let record = ApiKeyRecord::new(
            "key-1",
            "Automation",
            "hash-1",
            "ak_1234",
            ["management.read", "openapi.chat"],
            "admin",
        )
        .with_last_used_at("2026-05-17T00:00:00Z")
        .with_expires_at("unix:1800000000");

        repository
            .store_api_key(record.clone())
            .await
            .expect("api key should store");

        assert_eq!(
            repository
                .api_key_by_hash("hash-1")
                .await
                .expect("api key should load"),
            Some(record)
        );
        assert_eq!(
            repository
                .list_api_keys()
                .await
                .expect("api keys should list")
                .len(),
            1
        );
        assert!(
            repository
                .revoke_api_key("key-1", "2026-05-17T00:00:00Z".to_string())
                .await
                .expect("api key should revoke")
        );
        assert!(
            repository
                .api_key_by_hash("hash-1")
                .await
                .expect("api key should load")
                .expect("api key should exist")
                .is_revoked()
        );
        assert!(
            repository
                .delete_api_key("key-1")
                .await
                .expect("api key should delete")
        );
        assert!(
            repository
                .api_key_by_hash("hash-1")
                .await
                .expect("api key should load")
                .is_none()
        );
    }

    #[tokio::test]
    async fn sqlite_api_key_repository_persists_keys_across_instances() {
        let store = SqliteJsonStore::open_in_memory().expect("sqlite store should open");
        let repository = SqliteApiKeyRepository::new(store.clone());
        repository
            .store_api_key(
                ApiKeyRecord::new(
                    "key-1",
                    "Automation",
                    "hash-1",
                    "ak_1234",
                    ["management.read"],
                    "admin",
                )
                .with_last_used_at("2026-05-17T00:00:00Z"),
            )
            .await
            .expect("api key should store");

        let reloaded = SqliteApiKeyRepository::new(store);
        let record = reloaded
            .api_key_by_hash("hash-1")
            .await
            .expect("api key should load")
            .expect("api key should exist");
        assert_eq!(record.key_id, "key-1");
        assert_eq!(record.last_used_at.as_deref(), Some("2026-05-17T00:00:00Z"));
    }
}
