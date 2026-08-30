# test_builtin

A small Seen program that exercises printing, debugging, and assertions.

```bash
seen check src/main.seen
seen run src/main.seen
seen compile src/main.seen test_builtin
./test_builtin
```

The shipped 0.19.0 CLI does not provide project-wide `build` or `test`
subcommands; pass the source file to `check`, `run`, or `compile` explicitly.
