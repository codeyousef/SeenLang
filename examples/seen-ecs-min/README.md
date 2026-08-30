# seen-ecs-min

A deterministic entity-component-system micro-simulation. It models entities,
movement, health, and a small frame loop entirely in Seen; it is not an engine
or a platform-specific package.

From the repository root, use the verified compiler:

```bash
mkdir -p .seen/agent-tools/examples
scripts/run_with_project_artifacts.sh ecs-check -- \
  compiler_seen/target/seen check examples/seen-ecs-min/src/main.seen
scripts/run_with_project_artifacts.sh ecs-run -- \
  compiler_seen/target/seen run examples/seen-ecs-min/src/main.seen
scripts/run_with_project_artifacts.sh ecs-compile -- \
  compiler_seen/target/seen compile examples/seen-ecs-min/src/main.seen \
  .seen/agent-tools/examples/seen-ecs-min
.seen/agent-tools/examples/seen-ecs-min
```

Cross-compilation uses a shipped target name and a positional output path:

```bash
scripts/run_with_project_artifacts.sh ecs-android -- \
  compiler_seen/target/seen compile examples/seen-ecs-min/src/main.seen \
  .seen/agent-tools/examples/seen-ecs-min-android --target=android-arm64
```

That command requires the target toolchain documented in
[`docs/targets.md`](../../docs/targets.md); it does not create an Android app
bundle. The 0.19.0 compiler does not advertise a WebAssembly target.

`Seen.toml` records project metadata and preferred active targets. The direct
`check`, `run`, and `compile` commands still receive the source file explicitly.
