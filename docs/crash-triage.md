# Seen Crash Triage Playbook

This document describes the data we expect from production crashes and how to 
reproduce issues locally. Once PROD-4 instrumentation lands, we will embed
build-ids and runtime crash reports automatically.

## 1. Required Artifacts
- Build ID from the crashing binary (via `seen_cli doctor`)
- Crash report bundle (stack trace, IR block, scheduler metrics)
- ABI-compatible `seen_std` package

## 2. Reproduction Flow
1. Run `seen_cli doctor --dump-build-id <binary>` and verify IDs match the
   reported manifest.
2. Use `seen trace --replay <crash.json>` (planned) to re-run the failing IR.

## 3. Outstanding Work
- Embed `.note.seen.build_id` sections.
- Expand runtime crash hooks to record stack/scheduler metrics.
- Implement `seen trace --runtime/--replay` with deterministic logs.
- Wire this doc into the release checklist.

## Available Tools
- `seen doctor` prints version/git/timestamp metadata for the current CLI build.
- `seen doctor --dump-build-id <binary>` attempts to read the `.note.seen.build_id`
  note from an arbitrary binary and dumps it as hex (falls back to an informative message if the section is missing).
