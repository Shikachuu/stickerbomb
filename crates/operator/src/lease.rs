// Copyright 2026 Stickerbomb Maintainers
// SPDX-License-Identifier: Apache-2.0

//! Leader election components, using the k8s lease pattern.
//! <https://kubernetes.io/docs/concepts/architecture/leases/>

use std::{env, time::Duration};

use kube::Client;
use kube_leader_election::{LeaseLock, LeaseLockParams};
use tokio::{sync::watch, time::sleep};
use tracing::error;

/// Runs a leader election function using HOSTNAME as the lease name on the default namespace
/// infered from the k8s client, will notify through a watch.
pub async fn run_leader_election(client: Client, leader_tx: watch::Sender<bool>) {
    let holder_id = env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string());
    let namespace = client.default_namespace().to_string();

    let leadership = LeaseLock::new(
        client,
        &namespace,
        LeaseLockParams {
            holder_id,
            lease_name: "stickerbomb-lease".into(),
            lease_ttl: Duration::from_secs(15),
        },
    );

    loop {
        match leadership.try_acquire_or_renew().await {
            Ok(ll) => {
                let _ = leader_tx.send(ll.acquired_lease);
            }
            Err(err) => error!(error = err.to_string(), "failed to acquire lease lock"),
        }

        sleep(Duration::from_secs(5)).await;
    }
}
