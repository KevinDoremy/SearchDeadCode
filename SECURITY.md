# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.4.x   | :white_check_mark: |
| < 0.4   | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in SearchDeadCode, please report it responsibly.

### How to Report

1. **Do NOT open a public GitHub issue** for security vulnerabilities
2. Email the maintainer directly or use [GitHub's private vulnerability reporting](https://github.com/KevinDoremy/SearchDeadCode/security/advisories/new)
3. Include as much detail as possible:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

### What to Expect

- **Acknowledgment**: We will acknowledge receipt within 48 hours
- **Assessment**: We will assess the vulnerability and its impact within 7 days
- **Resolution**: Critical vulnerabilities will be addressed as quickly as possible
- **Disclosure**: We will coordinate with you on public disclosure timing

### Security Best Practices

When using SearchDeadCode:

- Always use the latest version
- Verify checksums of downloaded binaries
- Review the SBOM (Software Bill of Materials) attached to releases
- Verify SLSA provenance attestations for supply chain security

## Supply Chain Security

This project implements supply chain security best practices:

- **SLSA Level 2+ Provenance**: All releases include build provenance attestations
- **SBOM**: Software Bill of Materials generated for each release
- **Signed Releases**: Release artifacts are signed using Sigstore
- **OpenSSF Scorecard**: Continuous security assessment

## Dependencies

We regularly audit our dependencies using:
- `cargo audit` for known vulnerabilities
- Dependabot for automated security updates
- SBOM generation for transparency

## Security-Related Configuration

SearchDeadCode is a static analysis tool that:
- Only reads files (never modifies unless explicitly requested with `--remove`)
- Does not make network connections
- Does not execute analyzed code
- Respects `.gitignore` patterns

Thank you for helping keep SearchDeadCode secure!
