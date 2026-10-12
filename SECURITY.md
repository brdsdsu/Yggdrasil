# Security Policy

## Reporting a vulnerability

Please report security issues privately, **not** as a public issue.

Open a private report through GitHub Security Advisories: the repository's
**Security → Report a vulnerability** tab, or
[this link](https://github.com/brdsdsu/Yggdrasil/security/advisories/new).

Please include a description, reproduction steps (a minimal scene graph and the
call that misbehaves helps most), the affected commit, and the impact. We aim to
acknowledge reports within a week and to share a remediation timeline after
triage. Please give us a reasonable window to ship a fix before any public
disclosure.

## Supported versions

Yggdrasil has not cut a release yet. Security fixes land on `master`; please
reproduce against the latest `master` before reporting.

## Scope

This policy covers the crates in this repository (`scene-graph`,
`spatial-index`, `engine`, `python-binding`, `viz`, `bench`) and the
documentation book built from `docs/`.

- `spatial-index` is a vendored fork of [kiddo](https://github.com/sdd/kiddo).
  If the issue also exists upstream, please report it to kiddo as well.
- Vulnerabilities in third-party dependencies belong with their maintainers.
  Tell us too if Yggdrasil is affected in a way a dependency bump does not fix.
