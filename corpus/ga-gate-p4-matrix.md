# Isolith P4 GA gate - matrix (P4-M5-T03)

total=6 pass=6 fail=0 skip=0 (fidelity=2 | mbd=4)

| leg | id | status | detail |
|-----|----|--------|--------|
| fidelity | pmi-fidelity-matrix | PASS | matrix schema ok; cells=64 (PASS 14 / FAIL 2 / SKIP 48); FAIL cells are documented step-io capabilit |
| fidelity | isolith-box | PASS | hausdorff=0 vol_rel_err=0 eps=1.0e-09 |
| mbd | nist-ftc-06 | PASS | r=y4141 fail=2 warn=1 unverified=2 pass=12; r=prime fail=2 warn=1 unverified=2 pass=12 |
| mbd | nist-ftc-08 | PASS | r=y4141 fail=0 warn=1 unverified=3 pass=13; r=prime fail=0 warn=1 unverified=3 pass=13 |
| mbd | nist-stc-06 | PASS | r=y4141 fail=3 warn=1 unverified=2 pass=11; r=prime fail=3 warn=1 unverified=2 pass=11 |
| mbd | mbd-report | PASS | wrote ga-gate-p4-mbd-report.json and ga-gate-p4-mbd-report.md; summary rollup == granular (inputs=6  |
