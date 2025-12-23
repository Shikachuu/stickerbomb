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

/// Initialize the controller and shared state (given the crd is installed)
///
/// # Panics
/// Will panic if kube client cannot be initialized from the environment
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

#[instrument(skip(doc, ctx), fields(
    labeler_name = %doc.name_any(),
    labeler_namespace = doc.namespace().as_deref(),
    labeler_uid = tracing::field::Empty,
    resource_api = %doc.spec.resource_api,
    resource_kind = %doc.spec.resource_kind,
    has_rego_policy = doc.spec.rego.is_some(),
))]
#[allow(clippy::needless_pass_by_value, clippy::too_many_lines)]
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
        let needs_patch = patch.is_some();

        if can_patch && needs_patch {
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
                    &Patch::Merge(patch.unwrap()),
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
                can_patch = can_patch,
                needs_patch = needs_patch,
                reason = if can_patch { "labels_already_applied" } else { "rego_policy_rejected" },
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

/// Helper function to publish a Kubernetes event
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
