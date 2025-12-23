# Contributing

## Development

### Prerequisites

- a unix machine
- docker instance (this can be lima too with rootful docker)
- mise installed

### Quick Start

```bash
mise trust      # Make sure we allow mise to use our mise.toml file
mise install    # Install all project dependencies
mise run dev    # Bootstrap a k3d cluster and deploy a dev version using Tilt
```

### Workflow

1. Edit code
2. Tilt auto-rebuilds and redeploys (15-30s)
3. View logs and deployment status in Tilt UI: http://localhost:10350
4. Deploy some test resources by running `apply-samples` on Tilt UI or running `kubectl apply -f examples/sample-labeler.yaml`
5. Make sure `mise lint` always passes on the codebase and there are no changes after running it.

### Available commands

| Command            | Alias     | Description                                                                                            |
| ------------------ | --------- | ------------------------------------------------------------------------------------------------------ |
| `mise dev`         | `mise d`  | Starts up a local k3d envrionment with Tilt                                                            |
| `mise update-helm` | `mise uh` | Updates the helm chart's version, regenerates CRDs and json schemas from Rust code                     |
| `mise dev-down`    | `mise dd` | Destroys the local k3d cluster                                                                         |
| `mise lint`        |           | Lints the codebase with clippy and helm lint, templates the chart and validates the result kubeconform |
