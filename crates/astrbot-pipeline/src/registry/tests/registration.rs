use crate::PipelineStageRegistry;

use super::support::NamedStage;

#[test]
fn registry_rejects_duplicate_stage_types() {
    let mut registry = PipelineStageRegistry::new();
    registry
        .register_stage("dup", 10, || NamedStage("first"))
        .expect("first registration should work");

    let duplicate = registry.register_stage("dup", 20, || NamedStage("second"));

    assert!(duplicate.is_err());
}

#[test]
fn registry_rejects_blank_stage_types() {
    let mut registry = PipelineStageRegistry::new();

    let error = registry
        .register_stage("  ", 10, || NamedStage("blank"))
        .expect_err("blank stage type should be rejected");

    assert!(error.to_string().contains("must not be empty"));
}
