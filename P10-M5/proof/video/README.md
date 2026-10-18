# P10-M5-T02 - recorded install walkthrough (the "install video")

This directory holds the recorded fresh-install walkthrough for the P10 GA
gate, reproduced on the two approved machines (decision #9: U1 + a U1/VM-class
VM; the NAS is repo/storage only).

| file | what it is |
|------|------------|
| machine-a-online.typescript / .timing | `script -T` recording of the U1 online install (clean state -> one-command install.sh -> the 8 checklist rows) |
| machine-b-online.typescript / .timing | same walkthrough on machine B (keelwb2 U1/VM over SSH) |
| machine-a-online.mp4 | rendered replay of the U1 recording (10s) |
| machine-b-online.mp4 | rendered replay of the machine B recording, build span elided (13s) |
| keelworks-install-walkthrough.mp4 | combined A + B replay (the headline install video) |

Replay the typescripts yourself with `scriptreplay`:

```bash
scriptreplay -t machine-a-online.timing machine-a-online.typescript
```

The MP4s are terminal renderings of those typescripts (ANSI -> frames ->
libx264), produced with the small renderer at scripts/p10m5t02-video-render.py:

```bash
python3 scripts/p10m5t02-video-render.py machine-b-online.typescript \
    machine-b-online.timing machine-b-online.mp4
```

For machine B the long (50-minute) release build inside the recording is elided
in the rendered MP4 and replaced with a short marker; the typescript keeps the
full authentic session.

Both recordings show the same measured green signals: PDM
`/api/v1/health` 200 `{"status":"ok",...}`, viewer `/` 200, P2P `/healthz`
200, `oep-batch-convert` + `oep-render-worker` present in the image, and the
update checker off by default (no `OEP_UPDATE_CHECK=on` in the generated
.env). The measured gate rows are in ../ga-gate-p10-m5t02.{json,md}.
