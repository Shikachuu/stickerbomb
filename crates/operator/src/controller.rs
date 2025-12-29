// Copyright 2025 Stickerbomb Maintainers
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use std::time::Duration;

use crate::{Error, Result, telemetry};
use futures::StreamExt;
use k8s_openapi::api::core::v1::ObjectReference;
use k8s_openapi::chrono::Utc;
use kube::api::{DynamicObject, ListParams, ObjectMeta, Patch, PatchParams};
use kube::core::gvk::GroupVersion;
use kube::runtime::Controller;
use kube::runtime::events::{Event, EventType, Recorder};
use kube::runtime::watcher::Config;
use kube::{Api, Resource, ResourceExt, discovery};
use kube::{Client, runtime::controller::Action};
use regorus::Engine;
use serde_json::json;
use stickerbomb_crd::v1_alpha1::RegoRule;
use stickerbomb_crd::{Labeler, LabelerStatus};
use tokio::sync::RwLock;
use tracing::{Span, debug, error, field, info, instrument, warn};

use crate::diagnostics::Diagnostics;

/// Context for our reconciler
#[derive(Clone)]
pub struct Context {
    /// Kubernetes client
    pub client: Client,
    /// Diagnostics that contains the traces metrics and kube event recorder
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Kubernetes event emitter
    pub recorder: Recorder,
    /// In-memory status for the Labeler
    pub state: Arc<RwLock<LabelerStatus>>,
}

/// Holds the state of the whole application
#[derive(Clone, Default)]
pub struct State {
    /// Atomic lock for kubernetes diagnostics
    pub diagnostics: Arc<RwLock<Diagnostics>>,
}

impl State {
    /// Getter for diagnostics with read lock
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    /// Converts the application state to controller context
    pub async fn to_ctrl_context(&self, client: Client) -> Arc<Context> {
        let state = Arc::new(RwLock::new(LabelerStatus {
            resources_skipped: 0,
            resources_labeled: 0,
            resources_matched: 0,
        }));

        Arc::new(Context {
            recorder: self.diagnostics.read().await.recorder(client.clone()),
            client: client.clone(),
            state,
            diagnostics: self.diagnostics.clone(),
        })
    }
}

/// Instantiates and runs a new controller with it's dependencies from the current shared state.
///
/// # Panics
///
/// Panics if it cannot obtain a k8s api client.
#[instrument(skip(state))]
pub async fn run(state: State) {
    info!("initializing stickerbomb controller");

    // tokio will handle this?
    #[allow(clippy::expect_used)]
    let client = Client::try_default()
        .await
        .expect("failed to create kube client");

    info!("kubernetes client initialized successfully");

    let labelers = Api::<Labeler>::all(client.clone());
    if let Err(e) = labelers.list(&ListParams::default().limit(1)).await {
        error!(
            error = %e,
            "failed to list labeler resources, CRD may not be installed"
        );
        std::process::exit(1);
    }

    info!("labeler CRD verified, starting controller");

    Controller::new(labelers, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.to_ctrl_context(client).await)
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;

    info!("controller shutdown complete");
}

/// Main reconcile loop for the operator, recalls reconcile every 5 mins and processes a `Labeler`
/// instance with it's `Context`.
/// It fetches every resource that matches the `Labeler`'s kind and api, runs the rego condition if
/// specified and patches the resource labels if needed.
///
/// # Errors
///
/// This function will return an error if any of the k8s api calls fail, see `crate::Error` for
/// explicit error details.
#[instrument(skip(doc, ctx), fields(
    labeler_name = %doc.name_any(),
    labeler_namespace = doc.namespace().as_deref(),
    labeler_uid = tracing::field::Empty,
    resource_api = %doc.spec.resource_api,
    resource_kind = %doc.spec.resource_kind,
    has_rego_policy = doc.spec.rego.is_some(),
))]
#[allow(clippy::needless_pass_by_value)]
async fn reconcile(doc: Arc<Labeler>, ctx: Arc<Context>) -> Result<Action> {
    let name = doc.name_any();
    let oref = doc.object_ref(&());
    let uid = oref
        .uid
        .as_ref()
        .ok_or_else(|| "Unable to find objectId".to_string())?;

    let trace_id = telemetry::get_trace_id();
    if trace_id != opentelemetry::trace::TraceId::INVALID {
        Span::current().record("trace_id", field::display(&trace_id));
    }

    Span::current().record("labeler_uid", uid.as_str());

    info!("starting reconciliation");

    let (api, ar) = discover_target_resources(&doc, &ctx.client).await?;
    let resources = api.list(&ListParams::default()).await?;

    let total = i32::try_from(resources.items.len())?;
    info!(total_resources = total, "discovered target resources");

    let mut engine = regorus::Engine::new();
    let rego = doc.spec.rego.clone();

    handle_rego_rule(&mut engine, rego.as_ref(), uid)?;

    let mut resources_labeled = 0;
    let mut resources_skipped = 0;

    for resource in &resources {
        let target = resource.name_any();
        let target_namespace = resource.namespace();
        let kind = match &resource.types {
            Some(types) => types.kind.clone(),
            None => "resource".to_string(),
        };

        let can_patch = match &rego {
            Some(r) => {
                engine.set_input_json(&serde_json::to_string(&resource)?)?;
                engine.eval_bool_query(r.query.clone(), false)?
            }
            None => true,
        };

        let patch = patch_resource_labels(&doc, &resource.metadata);

        if can_patch {
            if let Some(patch_value) = patch {
                publish_event(
                    &ctx.recorder,
                    EventType::Normal,
                    "AdjustingLabels",
                    "Labeling",
                    Some(format!("Labeling {kind}: {target} with rule: {name}")),
                    &oref,
                )
                .await;

                let patch_api = if let Some(ns) = &target_namespace {
                    Api::namespaced_with(ctx.client.clone(), ns, &ar)
                } else {
                    api.clone()
                };

                patch_api
                    .patch(
                        &target,
                        &PatchParams::default(),
                        #[allow(clippy::unwrap_used)]
                        &Patch::Merge(patch_value),
                    )
                    .await?;

                debug!(
                    target_resource = %target,
                    "successfully patched resource"
                );

                resources_labeled += 1;
            } else {
                debug!(
                    target_resource = %target,
                    target_namespace = target_namespace.as_deref(),
                    target_kind = %kind,
                    reason = "labels_already_applied",
                    "skipping resource"
                );
                resources_skipped += 1;
            }
        } else {
            debug!(
                target_resource = %target,
                target_namespace = target_namespace.as_deref(),
                target_kind = %kind,
                reason = "rego_policy_rejected",
                "skipping resource"
            );
            resources_skipped += 1;
        }
    }

    {
        let mut state = ctx.state.write().await;
        state.resources_matched = total;
        state.resources_skipped = resources_skipped;
        state.resources_labeled = resources_labeled;
    }

    flush_state_to_api(&doc, &ctx).await?;

    publish_event(
        &ctx.recorder,
        EventType::Normal,
        "ReconciliationComplete",
        "Reconcile",
        Some(format!(
            "Labeled {resources_labeled} of {total} resources ({resources_skipped} skipped)"
        )),
        &oref,
    )
    .await;

    {
        let mut diag = ctx.diagnostics.write().await;
        diag.last_event = Utc::now();
    }

    info!(
        resources_matched = total,
        resources_labeled = resources_labeled,
        resources_skipped = resources_skipped,
        requeue_after_secs = 300,
        "reconciliation completed successfully"
    );

    Ok(Action::requeue(Duration::from_mins(5)))
}

/// Handles any error thrown by the reconcile function by reproting it to tracing and publishing a
/// failed event to the k8s events api, will requeue the reconcile in 1 minute.
#[instrument(skip(object, err, ctx), fields(
    labeler_name = %object.name_any(),
    labeler_namespace = object.namespace().as_deref(),
    error_type = ?err,
))]
#[allow(clippy::needless_pass_by_value)]
fn error_policy(object: Arc<Labeler>, err: &Error, ctx: Arc<Context>) -> Action {
    let err_msg = err.to_string();

    error!(
        error = %err_msg,
        requeue_after_secs = 60,
        "reconciliation failed, scheduling retry"
    );

    let ctx_clone = ctx.clone();
    let oref = object.object_ref(&());

    tokio::spawn(async move {
        publish_event(
            &ctx_clone.recorder,
            EventType::Warning,
            "ReconciliationFailed",
            "Reconcile",
            Some(format!("Error: {err_msg}")),
            &oref,
        )
        .await;
    });

    Action::requeue(Duration::from_mins(1))
}

/// Flushes the in-memory state from `Context` to `LabelerStatus` in the k8s api.
///
/// # Errors
///
/// This function will return an error if it's unable to obtain the resource's namespace or the
/// object unique name or if the patch or encode fails.
#[instrument(skip(doc, ctx), fields(
    labeler_name = doc.metadata.name.as_deref(),
    labeler_namespace = doc.metadata.namespace.as_deref(),
))]
async fn flush_state_to_api(doc: &Labeler, ctx: &Context) -> Result<Labeler> {
    let status = ctx.state.read().await.clone();
    let ns = &doc
        .namespace()
        .ok_or_else(|| Error::from("Unable to get source namespace".to_string()))?;
    let api: Api<Labeler> = Api::namespaced(ctx.client.clone(), ns);

    let name = doc
        .metadata
        .name
        .as_ref()
        .ok_or_else(|| "Object has a missing name".to_string())?;

    debug!(
        resources_matched = status.resources_matched,
        resources_labeled = status.resources_labeled,
        resources_skipped = status.resources_skipped,
        "flushing status to API server"
    );

    let status_patch = Patch::Merge(json!({"status": serde_json::to_value(status)?}));

    let result = api
        .patch_status(name, &PatchParams::default(), &status_patch)
        .await?;

    Ok(result)
}

/// Fetch every resource from the k8s api with the api kind and version defined in the provided `Labeler`.
///
/// # Errors
///
/// This function will return an error if it's unabled to pin the api group or kind.
#[instrument(skip(labeler, client), fields(
    resource_api = %labeler.spec.resource_api,
    resource_kind = %labeler.spec.resource_kind,
))]
async fn discover_target_resources(
    labeler: &Labeler,
    client: &Client,
) -> Result<(Api<DynamicObject>, discovery::ApiResource)> {
    let gv: GroupVersion = labeler.spec.resource_api.parse()?;
    let apigroup = discovery::pinned_group(client, &gv).await?;
    let (ar, _) = apigroup
        .recommended_kind(&labeler.spec.resource_kind)
        .ok_or_else(|| "Unable to find API kind".to_string())?;

    Ok((Api::all_with(client.clone(), &ar), ar))
}

/// Diffs any `ObjectMeta` with labels defined in a `Labeler` and will return the
/// diff in a k8s api format for a patch request or return `None` if there are no changes.
fn patch_resource_labels(labeler: &Labeler, meta: &ObjectMeta) -> Option<serde_json::Value> {
    let mut labels = meta.labels.clone().unwrap_or_default();
    let needs_update = labeler
        .spec
        .labels
        .iter()
        .any(|(k, v)| labels.get(k) != Some(v));

    if !needs_update {
        return None;
    }

    labels.extend(labeler.spec.labels.clone());

    Some(json!({
        "metadata": {
            "labels": labels
        }
    }))
}

/// Adds a new rego rule to the engine if needed.
///
/// # Errors
///
/// This function will return an error if it fails to add the rego rule to the engine.
fn handle_rego_rule(engine: &mut Engine, rule: Option<&RegoRule>, uid: &str) -> Result<()> {
    let Some(rule) = rule else {
        return Ok(());
    };

    let path = format!("{uid}.rego");

    if !engine.get_policies()?.iter().any(|r| *r.get_path() == path) {
        engine.add_policy(path, rule.policy.clone())?;
        info!("rego policy loaded successfully");
    }

    Ok(())
}

/// Helper function to publish a Kubernetes events.
/// Will swallow any error!
async fn publish_event(
    recorder: &Recorder,
    event_type: EventType,
    reason: impl Into<String>,
    action: impl Into<String>,
    note: Option<String>,
    oref: &ObjectReference,
) {
    let _ = recorder
        .publish(
            &Event {
                type_: event_type,
                reason: reason.into(),
                note,
                action: action.into(),
                secondary: None,
            },
            oref,
        )
        .await;
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use kube::client::Body;

    use super::*;

    #[test]
    fn test_patch_empty_resource_labels() {
        let om = ObjectMeta::default();
        let labeler = Labeler {
            metadata: ObjectMeta::default(),
            spec: stickerbomb_crd::v1_alpha1::LabelerSpec {
                resource_api: "v1".to_string(),
                resource_kind: "Pods".to_string(),
                rego: None,
                labels: BTreeMap::default(),
            },
            status: Some(LabelerStatus::default()),
        };

        assert_eq!(patch_resource_labels(&labeler, &om), None);
    }

    #[test]
    fn test_patch_resource_labels() {
        let mut labels = BTreeMap::new();
        labels.insert("myLabel".to_string(), "value".to_string());

        let om = ObjectMeta::default();
        let labeler = Labeler {
            metadata: ObjectMeta::default(),
            spec: stickerbomb_crd::v1_alpha1::LabelerSpec {
                resource_api: "v1".to_string(),
                resource_kind: "Pods".to_string(),
                rego: None,
                labels,
            },
            status: Some(LabelerStatus::default()),
        };

        assert_eq!(
            patch_resource_labels(&labeler, &om),
            Some(json!({"metadata": {"labels": {"myLabel": "value"}}}))
        );
    }

    #[test]
    fn test_handle_rego_rule() {
        let mut engine = regorus::Engine::new();
        let uid = "test";
        let rule = RegoRule {
            policy: r#"package stickerbomb
default allow = false
allow if {
    input.spec.resourceKind == "Pod"
}"#
            .to_string(),
            query: "data.stickerbomb.allow".to_string(),
        };

        assert_eq!(handle_rego_rule(&mut engine, None, uid).unwrap(), ());
        assert_eq!(handle_rego_rule(&mut engine, Some(&rule), uid).unwrap(), ());
        assert_eq!(engine.get_policies().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_discover_target_resources_with_mock() {
        use http::{Request, Response};
        use tower_test::mock;

        let (mock_service, mut handle) = mock::pair::<Request<Body>, Response<Body>>();
        let client = Client::new(mock_service, "default");

        let labeler = Labeler {
            metadata: ObjectMeta::default(),
            spec: stickerbomb_crd::v1_alpha1::LabelerSpec {
                resource_api: "v1".to_string(),
                resource_kind: "Pod".to_string(),
                rego: None,
                labels: BTreeMap::default(),
            },
            status: Some(LabelerStatus::default()),
        };

        tokio::spawn(async move {
            let (request, send) = handle.next_request().await.unwrap();
            assert!(request.uri().path().contains("/api/v1"));

            let api_resources = serde_json::json!({
                "kind": "APIResourceList",
                "apiVersion": "v1",
                "groupVersion": "v1",
                "resources": [{
                    "name": "pods",
                    "singularName": "pod",
                    "namespaced": true,
                    "kind": "Pod",
                    "verbs": ["get", "list", "watch", "create", "update", "patch", "delete"]
                }]
            });

            let response = Response::builder()
                .status(200)
                .body(Body::from(serde_json::to_vec(&api_resources).unwrap()))
                .unwrap();

            send.send_response(response);
        });

        let result = discover_target_resources(&labeler, &client).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_publish_event_with_mock() {
        use http::{Request, Response};
        use kube::runtime::events::Reporter;
        use tower_test::mock;

        let (mock_service, mut handle) = mock::pair::<Request<Body>, Response<Body>>();
        let client = Client::new(mock_service, "default");

        let reporter = Reporter {
            controller: "test-controller".into(),
            instance: Some("test-instance".into()),
        };

        let recorder = Recorder::new(client.clone(), reporter);

        let oref = ObjectReference {
            api_version: Some("v1".to_string()),
            kind: Some("Labeler".to_string()),
            name: Some("test-labeler".to_string()),
            namespace: Some("default".to_string()),
            uid: Some("test-uid".to_string()),
            ..Default::default()
        };

        tokio::spawn(async move {
            let (_request, send) = handle.next_request().await.unwrap();

            let response = Response::builder().status(201).body(Body::empty()).unwrap();

            send.send_response(response);
        });

        publish_event(
            &recorder,
            EventType::Normal,
            "TestReason",
            "TestAction",
            Some("Test note".to_string()),
            &oref,
        )
        .await;
    }
}
