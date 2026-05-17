use crate::ToolLoopPolicy;

#[test]
fn tool_loop_policy_normalizes_limits() {
    let policy = ToolLoopPolicy::default()
        .enabled()
        .with_max_steps(0)
        .with_timeout_seconds(0)
        .with_schema_mode("skills-like");

    assert_eq!(policy.max_steps, 1);
    assert_eq!(policy.tool_call_timeout_seconds, 1);
    assert_eq!(policy.schema_mode, "skills-like");
}
