# P8 GA gate report: multi-user edit/publish/release + concurrent lock test

- schema_version: 1
- generated_at: 2026-10-18T01:04:08.131029+00:00
- base_url: http://127.0.0.1:28893/api/v1

## Summary

| total | FAIL | SKIP | PASS |
|---|---|---|---|
| 11 | 0 | 0 | 11 |

## Rows

- **[PASS]** `client-00` (multiuser) -- user-00 edited+published+released item-3-802c703cb2ad
- **[PASS]** `client-01` (multiuser) -- user-01 edited+published+released item-1-0fbcd366a084
- **[PASS]** `client-02` (multiuser) -- user-02 edited+published+released item-2-d9179eb25687
- **[PASS]** `client-03` (multiuser) -- user-03 edited+published+released item-0-6886ad2b8ea3
- **[PASS]** `concurrent-lock-single-winner` (locks) -- exactly one of 1 racers held the lock: racer-01
- **[PASS]** `publish-gate` (locks) -- non-holder refused (409), racer-01 allowed (200)
- **[PASS]** `immutable-after-release` (immutable) -- released->working hop refused
- **[PASS]** `content-address-stable` (immutable) -- blob sha stable
- **[PASS]** `audit-log-present` (audit) -- append_only=True count=33
- **[PASS]** `audit-chain-verifies` (audit) -- head verified, count=33
- **[PASS]** `nas-bundle-sha256` (nas) -- NAS already-staged+re-hash: 34281160B re-verified sha256=dba4c21b6d2acb04 (put=None)
