# Required verification

The required workflow runs on `main`, `release/0.23.0`, or explicit dispatch.
Each run uses one required job, serial compiler/optimizer workers and a hard
memory scope derived from the machine's available memory. Swap is disabled.

Normal version-branch pushes queue behind the active run. A CI repair commit
whose message contains `[replace-ci]` deliberately replaces the older run when
its configuration is known to be obsolete. Main-branch pushes replace their
superseded run. None of these settings merge a branch or publish a release.

Serial acceptance has a four-hour deadline. The aggregate CI scope has a
five-hour-forty-five-minute deadline, and the job has a six-hour deadline.
Bootstrap, acceptance, proportional gates and artifact retention must fit those
bounds; increasing a wall-clock budget does not bypass a check or change its
memory, task, sanitizer, fuzz-duration or performance policy.

`seen-verification-logs-<commit>` retains build/guard log tails independently of
the larger frontend diagnostic artifact. Its index records original and retained
sizes, hashes and omitted files. Collection allows at most 128 files, 1 MiB per
file and 32 MiB in total, with bounded directory discovery and no symlink
following. This keeps a timeout's active gate and guard diagnostics accessible
without downloading generated LLVM IR and compiler executables.
