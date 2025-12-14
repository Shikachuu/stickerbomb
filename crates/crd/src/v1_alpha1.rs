//! v1Alpha1 CRD resources

use std::collections::HashMap;

use k8s_openapi::apimachinery::pkg::apis::meta::v1::{Condition, Time};
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Spec object for the `Labeler` CRD
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Default))]
#[kube(kind = "Labeler", group = "stickerbomb.dev", version = "v1alpha1")]
#[kube(status = "LabelerStatus", shortname = "doc")]
pub struct LabelerSpec {
    /// Describes the target api group of the target resource
    pub resource_api: String,
    /// Describes the target kind of the target resource
    pub resource_kind: String,
    /// Contains the labeling policy described in Rego
    pub rego_condition: Option<String>,
    /// List of labels to apply
    pub labels: HashMap<String, String>,
}

/// State object for the `Labeler` CRD
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct LabelerStatus {
    /// Number of resources that matched resource_kind
    pub resources_matched: i32,
    /// Number of resources labeled in last reconciliation
    pub resources_labeled: i32,
    /// Number of resources failed the rego condition evaluation
    pub resources_skipped: i32,
    /// Standard Kubernetes conditions (Ready, Error, etc.)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<Condition>,
    /// Timestamp of the last successful reconciliation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconcile_time: Option<Time>,
    /// Error message from last reconciliation attempt, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
