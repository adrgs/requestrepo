# Security Policy

## Reporting a Vulnerability

Please do not report security issues through public GitHub issues, pull requests, or social media.

Instead, open a private report through GitHub Security Advisories:

[Report a vulnerability](https://github.com/adrgs/requestrepo/security/advisories/new)

This creates a private thread visible only to you and the maintainers, and it lets us
coordinate a fix and a CVE before any details become public.

## What to Include

A good report makes triage much faster. Where possible, please provide:

- The affected component (HTTP server, DNS server, SMTP server, frontend, Docker setup)
- The version, commit hash, or `ghcr.io/adrgs/requestrepo` image tag you tested against
- Steps to reproduce, ideally with a minimal proof of concept
- The impact you believe the issue has, and any preconditions required to trigger it
- Any suggested remediation, if you have one in mind

## Scope

In scope:

- The code in this repository, including the Rust backend, the React frontend, and the
  Docker build
- The hosted instance at `requestrepo.com`

Out of scope:

- Findings that depend on a misconfigured self-hosted deployment, such as a weak
  `JWT_SECRET` or an exposed admin token
- Reports generated solely by automated scanners with no demonstrated impact
- Denial of service through raw traffic volume
- Missing hardening headers or similar issues without a concrete attack path

Note that RequestRepo is designed to receive arbitrary requests from anyone and to reflect
attacker-controlled content back to the session owner. Behaviour that is part of that design
is not by itself a vulnerability. If you believe a case crosses the line into an actual
security boundary being broken, please explain why in the report.

## Reporting an Abused Subdomain

If a `requestrepo.com` subdomain is being used for abuse, such as a malware beacon or a
phishing page, it does not take a vulnerability report to get it blocked:

- Open [requestrepo.com/abuse](https://requestrepo.com/abuse), or
- Send `POST https://requestrepo.com/api/v2/abuse/reports` with the subdomain, a category
  and a short description. The request format is in the
  [README](README.md#reporting-abuse).

A report blocks the subdomain at once: its requests are refused, its name stops resolving
and its owner can no longer use it. Only report a subdomain you have actually seen in
abusive traffic. The maintainers can lift a block that was made in error.

## Response Process

- We aim to acknowledge new reports within 72 hours
- We will confirm the issue and share an assessment of severity and a rough timeline
- We will keep you updated as the fix progresses, and let you know when it ships
- Once a fix is released, we publish the advisory and credit you, unless you prefer to stay
  anonymous

Researchers who have reported issues in the past are credited in the Security
Acknowledgments section of the [README](README.md).

## Supported Versions

Security fixes are applied to the `main` branch and published in the `latest` container
image. Older releases are not backported. Please make sure you are running current code
before reporting.
