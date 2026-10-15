# Precommit quality gate

One adaptive gate script, run in two places:

1. **Local**: as a git pre-commit hook that blocks commits until the fast
   deterministic gates pass.
2. **CI**: the exact same script held to a Gitea Actions workflow
   (`.gitea/workflows/precommit-gate.yaml`) on every push, so a commit that
   sidesteps the local hook is caught by the pipeline.

Vendored from the Hermes `precommit-quality-gate` skill (v1.1) so the repo is
self-contained and the gate is identical wherever it runs.

## What the gate checks (in order)

- Python syntax (`py_compile`) on staged `.py` files.
- `pytest` when the repo has a `pyproject.toml`, `pytest.ini`, or `tests/`.
- `cargo test` when a `Cargo.toml` exists.
- `go test ./...` when a `go.mod` exists.
- Impeccable design detector on staged UI files (`.tsx/.jsx/.html/.css/...`).
  Advisory only: design taste warnings do not block a commit.

Hard failures (syntax / tests) exit 1 and block the commit. The gate needs the
repo to be staged first (`git add`), because it reads staged files via
`git diff --cached`.

## Install the local hook

From the repo root:

```bash
bash scripts/gate/install-hook.sh
```

Symbolically links `.git/hooks/pre-commit` to this checkout's
`scripts/gate/precommit-gate.sh`, self-contained: it works on any machine,
clone, or worktree, without Hermes installed.

Remove the hook:

```bash
rm .git/hooks/pre-commit
```

## Run the gate without committing

```bash
bash scripts/gate/precommit-gate.sh
```

Optionally pass a repo directory: `bash scripts/gate/precommit-gate.sh /path/to/repo`.

## CI wiring

`.gitea/workflows/precommit-gate.yaml` installs the required toolchain in a
container, checks out the repo, and runs `bash scripts/gate/precommit-gate.sh`.
A red gate fails the pipeline. This is purposefully the same script that runs
on the local hook: no drift between what developers run and what CI enforces.
