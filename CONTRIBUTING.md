# Contributing

## Development

### Prerequisites

- Lima running with Docker
- mise installed

### Quick Start

```bash
mise install
mise run dev
```

### Workflow

1. Edit code in `crates/operator/src/`
2. Tilt auto-rebuilds and redeploys (15-30s)
3. View logs in Tilt UI: http://localhost:10350

### Testing

```bash
kubectl apply -f examples/sample-labeler.yaml
kubectl get pods --show-labels
```

### Debugging

```bash
kubectl port-forward -n stickerbomb-system deployment/stickerbomb 8080:8080
curl http://localhost:8080/
```
