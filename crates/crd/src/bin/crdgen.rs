//! Generates yaml CRD resources from rust code and prints it to stdout
use kube::CustomResourceExt;
use stickerbomb_crd::Labeler;

fn main() {
    print!("{}", serde_yaml::to_string(&Labeler::crd()).unwrap());
}
