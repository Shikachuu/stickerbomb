use k8s_openapi::chrono::{DateTime, Utc};
use kube::{
    Client,
    runtime::events::{Recorder, Reporter},
};
use serde::Serialize;

/// Diagnostics to be exposed by the web server
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    /// Last successful reconcile event
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    /// Kuberentes status reporter
    #[serde(skip)]
    pub reporter: Reporter,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "strickerbomb".into(),
        }
    }
}

impl Diagnostics {
    /// Creates a new recoreder wrapper around self
    #[must_use]
    pub fn recorder(&self, client: Client) -> Recorder {
        Recorder::new(client, self.reporter.clone())
    }
}
