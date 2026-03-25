# Security Policy

## Overview
This repository is maintained with security best practices appropriate for a public, open-source project. We appreciate responsible disclosure of security vulnerabilities and will make a best effort to address verified issues promptly.

## Supported Versions
Security fixes are provided for the following versions:

- **Default branch** (e.g., `main` / `master`): Supported
- **Most recent tagged release**: Supported
- **Older releases**: Best-effort support only (may be unsupported depending on severity and maintainer capacity)

If your organization requires longer support windows, please open a discussion or contact the maintainers to explore options.

## Reporting a Vulnerability
**Do not open public GitHub issues for suspected security vulnerabilities.**

Instead, use **one** of the following reporting mechanisms (preferred order):

1. **GitHub Private Vulnerability Reporting (recommended)**  
   Use the repository’s “Report a vulnerability” button (if enabled).

2. **Email**  
   Send a report to: **security@trademomentumllc.com**  
   (Replace this with a monitored mailbox.)

When reporting, please include:
- A clear description of the issue and the affected component(s)
- Steps to reproduce (proof-of-concept if available)
- Impact assessment (what an attacker can achieve)
- Affected versions / commit hashes
- Any known mitigations
- Your preferred contact information for follow-up

### What to Expect
After receiving a report, maintainers will:
- **Acknowledge receipt** within **3 business days** (best effort)
- **Triage** the report to confirm validity and severity
- Work on a fix and coordinate a release (as appropriate)

## Disclosure Policy
We follow a responsible disclosure process:
- We request that reporters **avoid public disclosure** until a fix is available or until an agreed disclosure date.
- Once resolved, we may publish a security advisory and credit the reporter (if desired).

## Security Update Process
When a vulnerability is confirmed, we typically:
1. Create an internal tracking item (private, if possible)
2. Develop and test a fix
3. Release patched versions and/or merge to the default branch
4. Publish notes (release notes or advisory) describing the impact and remediation steps

## Dependency and Supply Chain Security
This project aims to reduce supply-chain risk by:
- Keeping dependencies up to date where practical
- Reviewing dependency changes (especially major upgrades)
- Using automated tooling (e.g., Dependabot, Renovate, SCA) when enabled
- Avoiding committing secrets to the repository

## Secret Management
- **Never commit secrets** (API keys, private keys, tokens, passwords) to this repository.
- If a secret is accidentally committed:
  - Treat it as compromised
  - Rotate/revoke it immediately
  - Remove it from the repo history if required by policy (note: history rewrites can be disruptive)

## Secure Development Practices (Maintainers)
Maintainers should follow these practices when accepting changes:
- Require code review for non-trivial changes
- Prefer least-privilege access for CI/CD and cloud credentials
- Validate untrusted input and avoid unsafe deserialization
- Add or update tests for security-relevant changes
- Use CI to run linting, tests, and security scans where feasible

## Scope
This policy applies to:
- Source code and configuration in this repository
- Build and CI/CD workflows stored in this repository (e.g., GitHub Actions)

Out of scope unless explicitly stated:
- Third-party services or infrastructure not managed in this repository
- Forks or downstream distributions

## Policy Compliance
If this repository is used in an enterprise context, downstream adopters are responsible for:
- Their own risk assessment
- Configuration hardening
- Monitoring, patching, and incident response practices

## Contact
Security contact: **security@trademomentumllc.com**  
Maintainer contact (non-sensitive issues): open a GitHub issue or discussion.
