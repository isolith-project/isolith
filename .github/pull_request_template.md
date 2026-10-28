Thank you for contributing. Every field below has a purpose: the DCO line is
the legal gate, the card and gate locate the change in the program plan, and
the honesty checklist keeps every claim in this PR at the level a green test
proves. Fill all of it or the change is blocked until it is filled.

## What this changes

One or two sentences: what the change does, and why the program needs it.
A PR that cannot say this in two sentences is probably two changes and should
be split (one change at a time is program doctrine).

## Card and gate

- Card: (the card id this change belongs to, e.g. `t_ac965332`, or the issue
  number it closes)
- Gate / milestone: (the milestone or GA gate this change rolls up to, e.g.
  P13-T03, or "maintenance" if it is not milestone-tracked)
- Decision record: (if this change implements or modifies a decision, name
  the record in `docs/decision-records.md`; if written here, note "n/a")

## DCO sign-off

By submitting this pull request you certify the Developer Certificate of
Origin (see CONTRIBUTING.md): you authored the work or have the right to
submit it, and it is licensed to the project as stated in the charter.

Replace the placeholder below with your own sign-off line, exactly the same
as the one on your commits:

```
Signed-off-by: Your Name <you@example.com>
```

A raw placeholder left in place will pass the letter of the check. The
maintainer reviews the spirit too; put your real name and email.

## The test that proves it

Name the test or acceptance check that proves this change, and the command
that runs it. A claim without a command and a green result is a claim;
paste the relevant output line.

```
<command>
<proof line from the output>
```

## Honesty checklist

- [] This PR does not claim a status higher than a green accepted test proves
  (no "it opens everything", no parity claims without a test on real
  material, no format claim beyond `docs-site/format-matrix.json`).
- [] I ran the tests locally before pushing (`pytest tests/ -q`,
  `cargo test --workspace`) and they are green on this branch.
- [] No vendor binaries, no copied proprietary source, no material whose
  license does not allow contribution here (clean-room invariant).
- [] No credentials, environment files, keys, or generated build output are
  committed.
- [] Program prose contains no em dashes (U+2014) and no emojis.
- [] I used `git commit -s` on every commit in this branch.
