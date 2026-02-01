# Contributing

Thank you for your interest in contributing to Stickerbomb! This document provides guidelines and instructions for contributing to the project.

## Table of Contents

- [Development Setup](#development-setup)
- [Building the Project](#building-the-project)
- [Testing](#testing)
- [Code Standards](#code-standards)
- [Code Review Process](#code-review-process)
- [Security](#security)
- [Dependency Management](#dependency-management)
- [Release Process](#release-process)
- [Available Commands](#available-commands)

## Development Setup

### Prerequisites

- A Unix machine (Linux or macOS)
- Docker instance (this can be Lima too with rootful Docker)
- [mise](https://mise.jdx.dev/) installed

### Quick Start

```bash
mise trust      # Allow mise to use our mise.toml file
mise install    # Install all project dependencies
mise run dev    # Bootstrap a k3d cluster and deploy a dev version using Tilt
```

### Development Workflow

1. Edit code
2. Tilt auto-rebuilds and redeploys (15-30s)
3. View logs and deployment status in Tilt UI: http://localhost:10350
4. Deploy test resources by running `apply-samples` on Tilt UI or running `kubectl apply -f examples/sample-labeler.yaml`
5. Make sure `mise lint` always passes on the codebase and there are no changes after running it

## Building the Project

### Building the Rust Operator

You probably don't need this, just use the container image and the helm chart.

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release
```

### Building the Container Image

```bash
# Build the container image locally
docker build -t stickerbomb:dev .

# The Dockerfile uses multi-stage builds with distroless base images
# Base images are pinned by SHA for security and reproducibility
```

### Building the Helm Chart

```bash
# Update Helm chart version and regenerate CRDs
mise update-helm

# Package the chart
helm package charts/stickerbomb

# Push to GHCR OCI registry (requires authentication)
helm push stickerbomb-<version>.tgz oci://ghcr.io/shikachuu/charts
```

## Testing

We follow the [kube.rs testing pyramid](https://kube.rs/controllers/testing/) approach, ordered by cost and maintenance:

1. **Unit Tests** - Fast, isolated tests (pure logic + mocked Kubernetes API with `tower-test`)
2. **Integration & E2E Tests** - Run against real k3d clusters in separate `cluster-tests/` crate

### Running Tests

```bash
# Run all tests with coverage output
mise test

# Run tests with html coverage output
mise test-html

# Run integration & E2E tests (requires k3d cluster)
cargo test -p cluster-tests

# Run tests for specific crate (you rarely need these)
cargo test -p stickerbomb-operator
cargo test -p stickerbomb-crd
```

### Test Coverage Requirements

- **Coverage target:** 90% statement coverage (minimum 80% enforced by CI)
- All new code should include comprehensive tests
- Focus on testing error paths and edge cases
- Integration tests required for complex reconciliation logic

### Writing Tests

Follow the **testing pyramid**: focus on unit tests (pure and mocked) for most coverage, with fewer integration and E2E tests.

#### Unit Tests (Preferred)

Unit tests should be fast, isolated, and avoid external dependencies. We use two approaches:

**Pure unit tests** - Test business logic without Kubernetes dependencies using the **sans-IO pattern** (separate network calls from algorithms).

- **Examples:** See `crates/operator/src/controller.rs:435-492`
  - `test_patch_resource_labels` - label patching logic
  - `test_handle_rego_rule` - Rego policy loading

**Mocked unit tests** - Use [`tower-test::mock`](https://docs.rs/tower-test/) to test Kubernetes API interactions without a real cluster.

- **Examples:** See `crates/operator/src/controller.rs:494-584`
  - `test_discover_target_resources_with_mock` - mocking API discovery
  - `test_publish_event_with_mock` - mocking event publishing
- **Pattern:**

  ```rust
  use tower_test::mock;
  use http::{Request, Response};
  use kube::client::Body;

  let (mock_service, handle) = mock::pair::<Request<Body>, Response<Body>>();
  let client = Client::new(mock_service, "default");
  // Spawn handler, test function, verify behavior
  ```

#### Integration & E2E Tests

Tests that require a real Kubernetes cluster, organized in a separate `cluster-tests/` crate.

**Structure:**

```
cluster-tests/
  tests/
    integration/    # Direct reconcile calls
      reconcile.rs
    e2e/           # Black-box kubectl tests
      deployment.rs
```

**Integration tests** (call reconcile directly):

```rust
// cluster-tests/tests/integration/reconcile.rs
#[tokio::test]
async fn test_reconcile_flow() {
    let client = Client::try_default().await.unwrap();
    // Apply test resources, call reconciler, verify results
}
```

**E2E tests** (black-box via kubectl):

```rust
// cluster-tests/tests/e2e/deployment.rs
#[tokio::test]
async fn test_operator_deployment() {
    // Deploy operator, apply CRDs, verify behavior
}
```

### Testing Best Practices

1. **Focus on lower-level tests** - Unit and mocked tests provide best benefit-to-pain ratio
2. **Use descriptive names** - `test_patch_skips_resources_with_matching_labels` not `test_patch_1`
3. **Test both success and failure** - Especially error handling and edge cases
4. **Keep tests isolated** - Each test should be independent
5. **Mock liberally** - Use `tower-test` to avoid cluster dependencies
6. **Separate cluster tests** - Keep tests requiring real clusters in `cluster-tests/` crate

### Common Patterns

**Environment variables:**

```rust
temp_env::with_var("VAR_NAME", Some("value"), || { /* test */ });
```

See `crates/operator/src/telemetry.rs:82-156` for examples.

**Async testing:**

```rust
#[tokio::test]
async fn test_name() { /* async code */ }
```

**Further reading:**

- [kube.rs testing guide](https://kube.rs/controllers/testing/)
- [tower-test documentation](https://docs.rs/tower-test/)

## Code Standards

### Rust Style Guidelines

This project follows standard Rust conventions with additional requirements:

- **No unsafe code:** The project has `#![forbid(unsafe_code)]` - all unsafe code is prohibited
- **Clippy compliance:** All Clippy warnings must be addressed (treated as errors in CI)
- **Compiler warnings:** All compiler warnings are treated as errors in CI (`RUSTFLAGS="-D warnings"`)
- **Formatting:** Use `cargo fmt` - all code must be formatted before committing
- **Documentation:** Public APIs should have doc comments

For detailed coding standards, see `docs/CODING_STANDARDS.md`.

### License Headers

All source files must include SPDX license identifier and copyright notice!

**Automated with addlicense:** The project uses [addlicense](https://github.com/google/addlicense) to automatically add and verify license headers.
Running `mise lint` will check and add missing headers, thanks to this missing headers will also be reported by the CI.

This applies to only non `.toml` files in the `crates` folder which contains the application code.

## Code Review Process

### Pull Request Requirements

All pull requests must meet the following criteria:

1. **Tests pass:** All CI checks must pass (linting, tests, security scans)
2. **Coverage maintained:** Code coverage must not decrease
3. **Review approval:** At least 1 approval from a maintainer required
4. **No self-merge:** Authors cannot merge their own PRs
5. **Documentation updated:** Update relevant documentation for user-facing changes

### Review Checklist

Reviewers should verify:

- [ ] Code follows project coding standards
- [ ] Tests are comprehensive and meaningful
- [ ] Security implications have been considered
- [ ] Documentation is updated (if needed)
- [ ] No sensitive data or credentials are included
- [ ] Error handling is appropriate
- [ ] Performance impact is acceptable

## Security

### Security Review

All pull requests should consider security implications. Specific security review is required for:

- Changes to CRD validation logic
- Changes to Rego policy evaluation
- Changes to label patching logic
- Changes to RBAC/cluster role configurations
- New dependencies
- Changes to network communication

**Note on RBAC changes:** Any changes to `clusterRoles` configuration or default values must ensure that users are guided toward least-privilege configurations. The default permissive configuration should be clearly documented with strong security warnings.

### Reporting Security Issues

Do not open public issues for security vulnerabilities. Instead, please refer to [SECURITY.md](SECURITY.md) for instructions on how to report security issues privately.

For security review guidelines, see `docs/SECURITY_REVIEW_GUIDE.md`.

## Dependency Management

### License and Dependency Policy

This project uses [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) to ensure all dependencies use approved open-source licenses. The allowed licenses and dependency policies are defined in `deny.toml`.

**Approved licenses:**

- Apache-2.0
- MIT
- BSD (all variants)
- Other permissive FLOSS licenses as approved in `deny.toml`

When adding new dependencies:

1. Ensure they comply with the approved license list
2. Run `cargo deny check` to verify compliance
3. If adding a new license type, it **MUST** be a [FLOSS license](https://dwheeler.com/essays/floss-license-slide.html)
4. Update `deny.toml` if necessary with justification

### Dependency Updates

**Update process:**

- Dependabot automatically creates PRs for dependency updates
- All Dependabot PRs are automatically tested by CI
- Security updates should be merged within **7 days for critical**, **30 days for others**
- Review the changelog and test coverage before merging
- Major version updates require additional scrutiny

**Manual dependency updates:**

```bash
# Check for outdated dependencies
cargo outdated

# Update dependencies
cargo update
```

## Release Process

### Release Criteria

Before a release can be published, the following criteria must be met:

- [ ] All tests pass on main branch
- [ ] Test coverage meets threshold (80% minimum)
- [ ] No high or critical security vulnerabilities
- [ ] Security scans (CodeQL, Trivy) pass
- [ ] All CI checks pass
- [ ] CHANGELOG is up to date (auto-generated by Release Please)

### Versioning

This project follows [Semantic Versioning 2.0.0](https://semver.org/):

- **MAJOR:** Incompatible API changes
- **MINOR:** New functionality in a backwards-compatible manner
- **PATCH:** Backwards-compatible bug fixes

Releases are automated using Release Please based on [Conventional Commits](https://www.conventionalcommits.org/).

## Available Commands

| Command            | Alias     | Description                                                                                            |
| ------------------ | --------- | ------------------------------------------------------------------------------------------------------ |
| `mise dev`         | `mise d`  | Starts up a local k3d environment with Tilt                                                            |
| `mise update-helm` | `mise uh` | Updates the helm chart's version, regenerates CRDs and json schemas from Rust code                     |
| `mise dev-down`    | `mise dd` | Destroys the local k3d cluster                                                                         |
| `mise lint`        |           | Lints the codebase with clippy and helm lint, templates the chart and validates the result kubeconform |

## Questions or Need Help?

- Open an issue for bug reports or feature requests
- Check existing issues and documentation first
- Be respectful and constructive in all interactions

Thank you for contributing to Stickerbomb!
