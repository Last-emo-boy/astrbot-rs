use crate::validate_platform_id;

#[test]
fn platform_id_validation_rejects_blank_and_route_separators() {
    assert!(validate_platform_id("").is_err());
    assert!(validate_platform_id("   ").is_err());
    assert!(validate_platform_id("qq:default").is_err());
    assert!(validate_platform_id("qq!default").is_err());
    assert!(validate_platform_id("qq_default").is_ok());
}
