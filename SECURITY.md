# Security Policy

## Supported Versions

We provide security updates for the latest release only. If you're running an older version, please upgrade to the latest release to receive security patches.

| Version  | Supported          |
| -------- | ------------------ |
| Latest   | :white_check_mark: |
| < Latest | :x:                |

## Reporting a Vulnerability

### How to Report

**Please use GitHub Security Advisories** to report vulnerabilities:

1. Go to https://github.com/Shikachuu/stickerbomb/security/advisories
2. Click "Report a vulnerability"
3. Fill out the advisory form with as much detail as possible

**What to include in your report:**

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Any suggested fixes (optional)

### What to Expect

- **Acknowledgment:** We'll acknowledge your report within 48 hours
- **Updates:** We'll keep you informed as we investigate and work on a fix
- **Coordinated Disclosure:** We follow a 60-day coordinated disclosure timeline
  - You agree not to publicly disclose the vulnerability for 60 days
  - We'll work to patch and release a fix within this timeframe
  - If we need more time, we'll discuss an extension with you
- **Credit:** We'll credit you in the security advisory (unless you prefer to remain anonymous)

### Our Commitment

We aim to address vulnerabilities according to these timelines:

- **Critical severity:** Patch within 14 days
- **High severity:** Patch within 60 days
- **Medium severity:** Patch within 90 days
- **Low severity:** Best effort, typically next minor release

### Security Updates

When we release security patches, you'll see them in:

- **GitHub Security Advisories** (primary notification)
- **GitHub Releases** (release notes with `SECURITY:` prefix)
- **CHANGELOG.md** (automatically updated)

## Security Measures

Stickerbomb uses automated security scanning and follows secure development practices:

- **Automated Scanning:** CodeQL (SAST), Grype (vulnerability scanning), secret scanning, and license compliance checking
- **Review Process:** All security findings are reviewed by project maintainers before being closed
- **Dependencies:** Automated dependency updates via Dependabot with security monitoring

For details on our security architecture, see the existing [Security section in README.md](README.md#security).

### Deployment Security Best Practices

**IMPORTANT:** When deploying Stickerbomb, follow the principle of least privilege:

- **Restrict RBAC permissions:** The default configuration grants permission to patch ANY cluster resource. You should **always** configure `clusterRoles.rules` to only include the specific API groups and resources you need to label.
- **Review permissions regularly:** Periodically audit your `clusterRoles.rules` configuration to ensure you're not granting unnecessary permissions.
- **Use namespace-scoped resources when possible:** If you only need to label resources in specific namespaces, consider using namespace-scoped roles instead of cluster roles (requires custom deployment configuration).

See the [Security section in README.md](README.md#security) for configuration examples.

## Secrets Management

Stickerbomb itself does not require end users to manage any secrets for normal operation. The project handles secrets as follows:

- **Pipeline Secrets:** CI/CD pipeline secrets (GitHub tokens, registry credentials, signing keys, etc.) are managed exclusively by project maintainers with appropriate access controls
- **Secret Scanning:** Automated secret scanning is enabled to prevent accidental credential commits
- **No User Secrets Required:** The operator runs with Kubernetes RBAC permissions and does not require additional credentials or API keys from users

If you accidentally commit a secret to the repository, please report it immediately through our vulnerability reporting process above.

## Out of Scope

The following are generally **not** considered security vulnerabilities:

- **Kubernetes misconfigurations:** Issues arising from insecure cluster configurations or deployment settings (these are user responsibility)
- **Theoretical issues:** Vulnerabilities with no practical exploit path or that require unrealistic preconditions
- **Known dependency issues:** Vulnerabilities in dependencies that we're already tracking and working to address

If you're unsure whether something is in scope, feel free to report it anyway—we'd rather review it than miss a real issue!

## Questions?

For general security questions (not vulnerability reports), feel free to open an issue on GitHub. For actual vulnerabilities, always use the Security Advisory process above.

Thanks for helping keep Stickerbomb secure! 🔒
