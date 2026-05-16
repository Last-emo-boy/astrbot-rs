use crate::PipelineStageRegistry;

use super::support::NamedStage;

#[test]
fn registry_orders_stages_by_order_then_type() {
    let mut registry = PipelineStageRegistry::new();
    registry
        .register_stage("late", 20, || NamedStage("late"))
        .expect("late registration should work");
    registry
        .register_stage("same-b", 30, || NamedStage("same-b"))
        .expect("same-b registration should work");
    registry
        .register_stage("early", 10, || NamedStage("early"))
        .expect("early registration should work");
    registry
        .register_stage("same-a", 30, || NamedStage("same-a"))
        .expect("same-a registration should work");

    assert_eq!(
        registry.ordered_stage_types(),
        vec![
            "early".to_string(),
            "late".to_string(),
            "same-a".to_string(),
            "same-b".to_string(),
        ]
    );
}
