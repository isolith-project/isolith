# P10-M4-T01 multi-site replication: live two-host proof

- generated_at: 2026-10-18T08:07:33.788653+00:00
- lan_node: U1 (oep-pdm vault)
- remote_node: garage PC (sync client replica, WoL)
- deployment_role: Airgap (workstreams/P8-M5/hybrid-cloud-role-adr.md)
- binary: /home/r1a/keelworks-main/.worktrees/p10m4t01-replication-1017a/target/debug/oep-pdm (port 18789)

Summary: total=13 PASS=13 FAIL=0 SKIP=0

| id | leg | status | reader | detail |
|---|---|---|---|---|
| remote-health | replicate | PASS | remote-node | code=200 {'status': 'ok', 'service': 'oep-pdm', 'version': '1.0.0'} |
| remote-sees-lan-edit | replicate | PASS | remote-node | u1-rev blob=4ed71a0ab9.. vs expected 4ed71a0ab9.. |
| remote-edit-reaches-lan | replicate | PASS | remote-node | push code=201 status=synced |
| remote-divergence-detected | conflict | PASS | remote-node | push code=409 status=conflict |
| remote-three-way-snapshots | conflict | PASS | remote-node | base=63605d9092 ours=d03570d719 theirs=851aa19720 |
| remote-resolution-hash-verified | conflict | PASS | remote-node | resolved rev=3 |
| lan-edit-seen-on-remote | replicate | PASS | lan-node | node A edit revision=2 blob=bb7c33e3c284 |
| remote-edit-seen-on-lan | replicate | PASS | lan-node | sync_push status=synced code=201 |
| concurrent-edit-divergence | conflict | PASS | lan-node | push code=409 status=conflict |
| conflict-three-way-snapshots | conflict | PASS | lan-node | base=70e6252552 ours=0dab8c73ff theirs=d290f87088 |
| resolution-hash-verified | conflict | PASS | lan-node | resolved rev=3 blob=0dab8c73ff51 (choice=ours) |
| conflict-consumed | conflict | PASS | lan-node | GET conflict after resolve -> 404 expected, gone=True |
| airgap-lan-boundary | airgap | PASS | lan-node | requests seen: ['127.0.0.1:18789'] (bound host: 127.0.0.1:18789) |

Exit: 0 (0 = no FAIL)
