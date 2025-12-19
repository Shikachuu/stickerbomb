use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use k8s_openapi::chrono::Utc;
use kube::api::{DynamicObject, ListParams, ObjectMeta, Patch, PatchParams};
use kube::core::gvk::{GroupVersion, ParseGroupVersionError};
use kube::runtime::Controller;
use kube::runtime::events::{Event, EventType, Recorder};
use kube::runtime::watcher::Config;
use kube::{Api, Resource, ResourceExt, discovery};
use kube::{Client, runtime::controller::Action};
use serde_json::json;
use stickerbomb_crd::{Labeler, LabelerStatus};
use tokio::sync::RwLock;

use crate::diagnostics::Diagnostics;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Kube Error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("Serialization Error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Parse Error: {0}")]
    ParseError(#[from] ParseGroupVersionError),

    #[error("{0}")]
    Message(String),

    #[error("Rego Error: {0}")]
    AnyhowError(#[from] anyhow::Error),
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::Message(msg)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

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

#[derive(Clone, Default)]
pub struct State {
    pub diagnostics: Arc<RwLock<Diagnostics>>,
}

impl State {
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

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
pub async fn run(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let labelers = Api::<Labeler>::all(client.clone());
    if let Err(_e) = labelers.list(&ListParams::default().limit(1)).await {
        std::process::exit(1);
    }

    Controller::new(labelers, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.to_ctrl_context(client).await)
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

async fn reconcile(doc: Arc<Labeler>, ctx: Arc<Context>) -> Result<Action> {
    let name = doc.name_any();
    let oref = doc.object_ref(&());
    let uid = oref
        .uid
        .as_ref()
        .ok_or_else(|| "Unable to find objectId".to_string())?;

    let api = discover_target_resources(&doc, &ctx.client).await?;
    let resources = api.list(&Default::default()).await?;

    let mut engine = regorus::Engine::new();
    let rego = doc.spec.rego.clone();

    if rego.is_some() {
        let path = format!("{uid}.rego");
        let has_rule = engine
            .get_policies()?
            .iter()
            .any(|rule| *rule.get_path() == path);
        if !has_rule {
            engine.add_policy(path, rego.as_ref().unwrap().policy.clone())?;
        }
    }

    let mut resources_labeled = 0;
    let mut resources_skipped = 0;

    for resource in &resources {
        let target = resource.name_any();
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

        if can_patch && patch.is_some() {
            ctx.recorder
                .publish(
                    &Event {
                        type_: EventType::Normal,
                        reason: "AdjustingLabels".to_string(),
                        note: Some(format!("Labeling {kind}: {target} with rule: {name}")),
                        action: "Labeling".to_string(),
                        secondary: None,
                    },
                    &oref,
                )
                .await
                .ok();

            api.patch(
                &target,
                &PatchParams::default(),
                &Patch::Merge(patch.unwrap()),
            )
            .await?;

            resources_labeled += 1;
        } else {
            resources_skipped += 1;
        }
    }

    let total = resources.items.len() as i32;

    {
        let mut state = ctx.state.write().await;
        state.resources_matched = total;
        state.resources_skipped = resources_skipped;
        state.resources_labeled = resources_labeled;
    }

    flush_state_to_api(&doc, &ctx).await?;

    ctx.recorder
        .publish(
            &Event {
                type_: EventType::Normal,
                reason: "ReconciliationComplete".to_string(),
                note: Some(format!(
                    "Labeled {} of {} resources ({} skipped)",
                    resources_labeled, total, resources_skipped
                )),
                action: "Reconcile".to_string(),
                secondary: None,
            },
            &oref,
        )
        .await
        .ok();

    {
        let mut diag = ctx.diagnostics.write().await;
        diag.last_event = Utc::now();
    }

    Ok(Action::requeue(Duration::from_mins(5)))
}

fn error_policy(object: Arc<Labeler>, err: &Error, ctx: Arc<Context>) -> Action {
    let err_msg = err.to_string();
    let ctx_clone = ctx.clone();
    let oref = object.object_ref(&());

    tokio::spawn(async move {
        let _ = ctx_clone
            .recorder
            .publish(
                &Event {
                    type_: EventType::Warning,
                    reason: "ReconciliationFailed".to_string(),
                    note: Some(format!("Error: {}", err_msg)),
                    action: "Reconcile".to_string(),
                    secondary: None,
                },
                &oref,
            )
            .await;
    });

    Action::requeue(Duration::from_mins(1))
}

async fn flush_state_to_api(doc: &Labeler, ctx: &Context) -> Result<Labeler> {
    let status = ctx.state.read().await.clone();
    let api: Api<Labeler> = Api::all(ctx.client.clone());

    let name = doc
        .metadata
        .name
        .as_ref()
        .ok_or_else(|| "Object has a missing name".to_string())?;

    let status_patch = Patch::Merge(json!({"status": serde_json::to_value(status)?}));

    let result = api
        .patch_status(name, &PatchParams::default(), &status_patch)
        .await?;

    Ok(result)
}

async fn discover_target_resources(
    labeler: &Labeler,
    client: &Client,
) -> Result<Api<DynamicObject>> {
    let gv: GroupVersion = labeler.spec.resource_api.parse()?;
    let apigroup = discovery::pinned_group(client, &gv).await?;
    let (ar, _) = apigroup
        .recommended_kind(&labeler.spec.resource_kind)
        .ok_or_else(|| "Unable to find API kind".to_string())?;

    Ok(Api::all_with(client.clone(), &ar))
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
