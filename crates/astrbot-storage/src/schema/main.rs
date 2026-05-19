use super::conversation::conversation_tables;
use super::ops::ops_tables;
use super::persona_skill::persona_skill_tables;
use super::platform::platform_tables;
use super::provider::provider_tables;
use super::{StorageSchema, StorageTable};

pub fn astrbot_main_v4() -> StorageSchema {
    StorageSchema::new("main_db", 4, astrbot_main_v4_tables())
}

pub fn astrbot_main_v4_tables() -> Vec<StorageTable> {
    let mut tables = Vec::new();
    tables.extend(conversation_tables());
    tables.extend(provider_tables());
    tables.extend(platform_tables());
    tables.extend(persona_skill_tables());
    tables.extend(ops_tables());
    tables
}
