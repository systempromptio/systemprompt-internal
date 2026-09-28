//! The installation-wide `organization` subject the gateway's daily quota
//! window keys on.

use systemprompt_security::authz::ROLE_PRECEDENCE;
use systemprompt_web_admin::authz::group::group_dimension;
use systemprompt_web_admin::authz::organization::{
    ORGANIZATION_DEFAULT, organization_dimension, organization_rule_type,
};

#[test]
fn organization_is_the_widest_band_and_names_the_policy_subject() {
    let dimension = organization_dimension();
    assert_eq!(dimension.rule_type, organization_rule_type());
    assert_eq!(
        organization_rule_type().as_str(),
        "organization",
        "must equal the `subject:` in services/gateway/policies.yaml or the window faults"
    );
    assert!(
        dimension.precedence > ROLE_PRECEDENCE
            && dimension.precedence > group_dimension().precedence,
        "everyone holds it, so it must lose to every narrower statement"
    );
    assert_eq!(ORGANIZATION_DEFAULT, "default");
}
