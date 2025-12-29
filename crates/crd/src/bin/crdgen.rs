// Copyright 2025 Stickerbomb Maintainers
// SPDX-License-Identifier: Apache-2.0

//! Generates yaml CRD resources from rust code.
//! By default this will target the helm chart's `crds` directory!
//! Designed to be used inside of a mise command that sets the `CRDS_DIR` and `SCHEMA_DIR` environment variables.
use std::{fs::File, io::Write, path::Path};

use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::CustomResourceExt;
use stickerbomb_crd::Labeler;

#[allow(clippy::unwrap_used)]
fn generate_crd_files(crd: &CustomResourceDefinition, crds_dir: &Path, schema_dir: &Path) {
    let kind = crd.spec.names.kind.to_lowercase();

    let yaml = serde_yaml::to_string(&crd).unwrap();
    let yaml_path = crds_dir.join(format!("{kind}-crd.yaml"));
    File::create(yaml_path)
        .unwrap()
        .write_all(yaml.as_bytes())
        .unwrap();

    let version = &crd.spec.versions[0];
    let openapi_schema = version
        .schema
        .as_ref()
        .and_then(|s| s.open_api_v3_schema.as_ref())
        .unwrap();

    let schema_json: serde_json::Value = serde_json::to_value(openapi_schema).unwrap();

    let api_version = format!("{}/{}", crd.spec.group, version.name);
    let full_schema = serde_json::json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "required": ["apiVersion", "kind", "metadata", "spec"],
        "properties": {
            "apiVersion": {
                "type": "string",
                "const": api_version
            },
            "kind": {
                "type": "string",
                "const": crd.spec.names.kind
            },
            "metadata": {
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "namespace": { "type": "string" }
                },
                "required": ["name"]
            },
            "spec": schema_json["properties"]["spec"],
            "status": schema_json["properties"]["status"]
        }
    });

    let json = serde_json::to_string_pretty(&full_schema).unwrap();
    let json_path = schema_dir.join(format!("{kind}_{}.json", version.name));
    File::create(json_path)
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();
}

#[allow(clippy::unwrap_used)]
fn main() {
    let crds_dir_str = std::env::var_os("CRDS_DIR").unwrap();
    let schema_dir_str = std::env::var_os("SCHEMA_DIR").unwrap();

    let crds_dir = Path::new(&crds_dir_str);
    let schema_dir = Path::new(&schema_dir_str);

    // Add your CRDs here
    let crds = vec![Labeler::crd()];

    for crd in crds {
        generate_crd_files(&crd, crds_dir, schema_dir);
    }
}
