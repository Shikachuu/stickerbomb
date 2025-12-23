//! v1Alpha1 CRD resources

use std::collections::BTreeMap;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `RegoRule` represents the optional rego policy and query for the condition evaluation
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegoRule {
    /// Policy defines the rego policy that will be used in the engine
    #[schemars(length(min = 1, max = 65536))]
    pub policy: String,
    /// Query defines the rego query the engine will evaluate
    #[schemars(length(min = 1, max = 1024))]
    pub query: String,
}

/// Spec object for the `Labeler` CRD
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Default))]
#[serde(rename_all = "camelCase")]
#[kube(kind = "Labeler", group = "stickerbomb.dev", version = "v1alpha1")]
#[kube(status = "LabelerStatus", shortname = "doc")]
#[kube(namespaced)]
pub struct LabelerSpec {
    /// Describes the target api group of the target resource (e.g., "v1", "apps/v1", "cert-manager.io/v1")
    #[schemars(length(min = 1, max = 253))]
    #[schemars(regex(
        pattern = r"^([a-z0-9]([a-z0-9.-]*[a-z0-9])?/)?[a-z0-9]([a-z0-9-]*[a-z0-9])?$"
    ))]
    pub resource_api: String,
    /// Describes the target kind of the target resource (e.g., "Pod", "Deployment")
    #[schemars(length(min = 1, max = 63))]
    #[schemars(regex(pattern = r"^[A-Z][a-zA-Z0-9]*$"))]
    pub resource_kind: String,
    /// Contains the labeling policy described in Rego
    pub rego: Option<RegoRule>,
    /// List of labels to apply (must contain at least one label)
    #[schemars(length(min = 1))]
    pub labels: BTreeMap<String, String>,
}

/// State object for the `Labeler` CRD
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LabelerStatus {
    /// Number of resources that matched `resource_kind`
    #[schemars(range(min = 0))]
    pub resources_matched: i32,
    /// Number of resources labeled in last reconciliation
    #[schemars(range(min = 0))]
    pub resources_labeled: i32,
    /// Number of resources failed the rego condition evaluation
    #[schemars(range(min = 0))]
    pub resources_skipped: i32,
}
