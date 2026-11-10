# test_builtin

A small Seen program that exercises printing, debugging, and assertions.

```bash
seen check src/main.seen
seen run src/main.seen
seen compile src/main.seen test_builtin
./test_builtin
```

Pass the source file to `check`, `run`, or `compile` explicitly. Current 0.23.0
development also provides canonical `seen test <project>`; published 0.22.11
does not. Neither compiler provides the legacy project-wide `build` command.
