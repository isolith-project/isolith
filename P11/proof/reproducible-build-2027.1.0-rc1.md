# Keelworks reproducible build proof -- 2027.1.0-rc1

Toolchain: rustc 1.98.1 (48a229cea 2026-09-01)  |  Source date epoch: 1700000000  |  Host: promaxgb10-83f1  |  Built: 2026-10-18T19:37:17Z

Two clean-room release builds (isolated CARGO_TARGET_DIR, incremental off,
source paths remapped) produced 12 artifacts: all byte-identical: yes.

- oep-batch-convert  sha256 3beb97fdd0c9de6980ebdfa4e9fbf946efaaf35a788941682cf8fd0f2366401a  identical yes
- oep-farm-drill  sha256 068e69168dfef3be66ac493d3bab9020b58506bba6dc929c838fa1819b7f154e  identical yes
- oep-ga-gate  sha256 1e6e569fa0434bd11efc5a42de7530727f721a4ae88e79fd55a92444108b6277  identical yes
- oep-ga-node  sha256 bf681951e559910466641b7463029e118a400f42688572cf79673435595dc143  identical yes
- oep-opsdash  sha256 9bd23336bd4651edbf98ec048da5db4474af6e4ceaf7ee937f1079d1f5524350  identical yes
- oep-pdm  sha256 f54638812804d5cc051b75e2b9d5d497a564845bc9470e6314247bf320c1a7f8  identical yes
- oep-pdm-stub  sha256 f66d697eb1e9c67b29ff3637007ec664bdae91db8caf8411df296d32b747a217  identical yes
- oep-render-worker  sha256 1fbc63c7d474db04fca8892be6e20ce901f209dc279cc108e50989bd83e57421  identical yes
- oep-update-check  sha256 d9c4e2bcc82a8da43a6faf3fbb65b8d56c20d37ff63107c56a7ea92147c4ce46  identical yes
- oep-update-sign  sha256 8436bd87f8c6ae2441309908c4ff44300e198eaed0b2c98c43e608b6597b4938  identical yes
- oep-web  sha256 78620ec9d1b8ac6f98c936172c18499f151428e701b86e5d5e62138b414688b8  identical yes
- sbox-probe  sha256 957c92ceeb5db8d42f3808bdd066bd2ed00f6ce0f1bc9b7bdb7c80554b0797a4  identical yes

Artifact manifest (P11-T04 signing input): P11/proof/reproducible-build-2027.1.0-rc1.SHA256SUMS
