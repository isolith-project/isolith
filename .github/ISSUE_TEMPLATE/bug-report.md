---
name: Bug report
about: Report a defect in a shipped or in-development build, with a repro
title: "[bug] "
labels: bug
---

## Bug report

Thank you for reporting. A good bug report reproduces the problem on a named build; it does not guess a cause. Fill in each field that applies.

### Environment

- Isolith release train or branch: (e.g. 2027.1, 2027.1.1, or `main @ <commit sha>`)
- Component: (e.g. `oep-pdm`, `oep-web`, `oep-opsdash`, `oep-sandbox`, `oep-cgr`, `oep-3dxml`, `oep-step`, ...)
- Component version (if a crate): (semver version)
- Deployment: (containers or from source; LAN/air-gap if relevant)

### Steps to reproduce

1. ...
2. ...
3. ...

### Expected behavior

What you expected to happen, and which document or test states that expectation (a README, the docs site, the format matrix, an acceptance test).

### Observed behavior

What actually happened. Include the exact error text, exit code, or log line.

### Repro material

If a real file triggers it, say what it is and where it came from (you do not have to attach it; a file name plus the part of the file that matters is often enough). A proposed acceptance test or a minimal failing input is the fastest path to a fix.

---

Honesty rule: report the behavior you observed on the build you tested. Do not claim a severity, a root cause, or a fix path that your reproduction does not prove; a labeled guess is fine, an unlabeled one is not.
