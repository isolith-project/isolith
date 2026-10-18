# P9-M5-T02 -- P9 GA gate

Run: 2026-10-18T03:40:01Z

**Summary:** total=5 PASS=5 FAIL=0 SKIP=0

## Rows

| leg | id | status | class | detail |
|-----|----|--------|-------|--------|
| BOOT | vault-boot | PASS |  | real oep-pdm on http://127.0.0.1:28913 (persist /home/r1a/.hermes/cache/scratch/p9ga-proof-1loYja) |
| WAVE | wave | PASS |  | 100-job wave: total=100 ok=100 skip=0 fail=0 hash_verified=100 nodes=3/3 |
| PDM | pdm | PASS |  | all 100 outputs re-hashed in the vault |
| NODES | nodes | PASS |  | 3 distinct nodes executed jobs |
| AUDIT | audit | PASS |  | tamper-evident audit chain re-verified |
