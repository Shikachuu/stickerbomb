//! Generates yaml CRD resources from rust code.
//! By default this will target the helm chart's `crds` directory!
//! Designed to be used inside of a mise command that sets the `CRDS_DIR` environment variable.
use std::{fs::File, io::Write, path};

use kube::CustomResourceExt;
use stickerbomb_crd::Labeler;

#[allow(clippy::unwrap_used)]
fn main() {
    let labeler_schema = serde_yaml::to_string(&Labeler::crd()).unwrap();
    let labeler_crd_path =
        path::Path::new(&std::env::var_os("CRDS_DIR").unwrap()).join("labeler-crd.yaml");
    let mut labeler_file = File::create(labeler_crd_path).unwrap();
    labeler_file.write_all(labeler_schema.as_bytes()).unwrap();
}
