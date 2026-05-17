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
    let conversation = conversation_tables();
    let provider = provider_tables();
    let platform = platform_tables();
    let persona_skill = persona_skill_tables();
    let ops = ops_tables();

    vec![
        conversation[0].clone(),
        conversation[1].clone(),
        provider[0].clone(),
        conversation[2].clone(),
        provider[1].clone(),
        platform[0].clone(),
        platform[1].clone(),
        persona_skill[0].clone(),
        persona_skill[1].clone(),
        platform[2].clone(),
        platform[3].clone(),
        ops[0].clone(),
        ops[1].clone(),
        platform[4].clone(),
        platform[5].clone(),
        persona_skill[2].clone(),
        persona_skill[3].clone(),
    ]
}
