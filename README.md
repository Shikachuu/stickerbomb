# Stickerbomb

[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/11661/badge)](https://www.bestpractices.dev/projects/11661)
[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![SAST](https://github.com/Shikachuu/stickerbomb/workflows/SAST/badge.svg?branch=main)](https://github.com/Shikachuu/stickerbomb/actions/workflows/codeql.yaml)

Stickerbomb is a kubernetes operator for labeling resources, imagine like putting little stickers on them, hence the name.

The use cases stickerbomb is made for:

- Statically labeling tons of freshly deployed or **already existing** resources.
  ("I want to have `istio-injection: enabled` label on all my namespaces.")
- Dynamically labeling tons of various resources based on the object's json schema.
  ("I want to have `image-source: quay` label on my pods that has a container image downloaded from quay.")
- Very complex use cases where you wish to label resources based on complex rego conditions.
  ("I want to have `component: networking` on all resources that has an open port that identifies a networking resource for example `53` for dns.")

## Getting started

- Install the official helm chart to your cluster:
  ```bash
  helm install stickerbomb oci://ghcr.io/shikachuu/charts/stickerbomb --version 0.1.0
  ```
  Wait for the tests to pass, it will make sure stickerbomb can actually label stuff.
- Get some context on the target resource you wish to label. Stikerbomb uses you the target resource's json object as input for your rego conditions,
  it's always a great idea to have some solid understaindg of the data you can use, run `kubectl get <resourceKind> <objectName> -o json` to check the json representation.
- Create a `Labeler` resource, you can find plenty of examples in the `examples` directory.
- Check the reconcile loop's status from events `kubectl events` or from logs.

## Internals

The main goal of this project to conditionally or unconditionally label any kubernetes resource that you can define in CRDs.
CRDs are an excelent way of providing a declarative configuration resource with strict validation with an operator that runs
a reconcile loop very often will prevent state drifts, so it's not a label once and hope it stays there model, this makes Stickerbomb a perfect GitOps capable operator.

Rust has been choosen as the primary language because of the maturity in the K8s ecosystem, esepically `kube-rs`,
while `kubert` would have been a great alternative, it's not made for write heavy operators.

Helm is our primary templating/distribution tool of choice, it has some great CRD capabilities and support for OCI registries and provenance checking.

Mise is used as a Makefile replacement and dependency management tool, it provides great support for various shell environments in unix systems with really grate task capabilites.

## Configuration

Stickerbomb was made to ship as a helm chart, so all the configuration paramteres for the operator sits in the `values.yaml` file, even the local development uses helm.

## Observability

Stickerbomb has opentelemetry traces and logs.

You can use structured logging with json by switching `operator.logFormat` to `json` in the values file and change the level by modifying `operator.logLevel`.

You can use the opentelemtry traces by setting the envrionment variable `OTEL_EXPORTER_OTLP_ENDPOINT` from the semantic convention in the values file.
To provide a more streamlined interface for this you can set the value of this env var by changing `operator.otel.endpoint` in the values file too.

## Security

**For vulnerability reporting and our security policy, see [SECURITY.md](SECURITY.md).**

- We provide signed helm releases in Github's OCI repository with signed images as well.
- By default stickerbomb ships as a rootless and distroless container do minimize the risk of CVEs in container images.
  While it limits our debugging capabilites provides a solid base.
- For every release we provide SBOMs so you can continously check for CVEs any point in time for your running release.
  (Both for the binary and for the container images.)
- The helm chart is sipped with a strick network policy that will always be installed if you have the `NetworkPolicy` capability in your cluster.
  It denies every egress and ingress calls from the pod except egress to port `443` and `53` targeting the `kube-system` namespace and ingress traffic on `8080`.
- The operator's deployment always run as non-root with fully dropped capabilites!
- Stickerbomb ships with its own service account with cluster role bindings. **By default, the operator has permission to patch ANY resources in the cluster** (using empty string `""` for both `apiGroups` and `resources` in `clusterRoles.rules`).

  **STRONGLY RECOMMENDED:** Restrict the operator's permissions to only the resource types you actually need to label. Configure `clusterRoles.rules` with explicit API groups and resources following the principle of least privilege:
  ```yaml
  clusterRoles:
    rules:
      - apiGroups: [""]  # core API group
        resources: ["pods", "namespaces"]
      - apiGroups: ["apps"]
        resources: ["deployments", "statefulsets"]
  ```
  This minimizes the security impact if the operator is compromised and ensures it can only modify the resources you explicitly allow.
