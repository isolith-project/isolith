# P11-T02 deliberate-failure repro

timestamp: 2026-10-19T06:59:47Z

A release tree whose Cargo.lock carries a seeded critical vulnerability
(in a dependency; a test dependency is resolved into Cargo.lock like any
other) MUST fail the release gate (exit 1).

| leg | seed | advisory | gate rc (expected 1) |
|---|---|---|---|
| hermetic | keelworks-fixture-unsafe@1.5.0 | RUSTSEC-0000-2026-CRIT critical (fixture db) | 1 |
| live | (not run; pass --advisory-db) | n/a | n/a |

Result: PASS - the gate failed on every seeded tree.

