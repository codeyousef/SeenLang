# Development registry smoke package

This source-only package is the immutable publish fixture for the development
registry. Its archive contains the byte-exact `Seen.toml`, MIT license, and Seen
source committed under this directory.

Publish only after an immutable tag for the commit is available from GitHub. Bind the request
to the repository's numeric GitHub ID, the numeric GitHub App installation ID,
the full tag ref, and the exact 40-character commit ID:

```bash
seen pkg publish examples/registry-smoke \
  --registry https://seen.dev.yousef.codes/packages \
  --token-file <mode-0600-token-file-outside-this-directory> \
  --source-forge github \
  --source-repository-id <numeric-repository-id> \
  --source-installation-id <numeric-installation-id> \
  --source-ref refs/tags/registry-smoke-v0.1.1 \
  --source-commit <full-commit-id> \
  --license-spdx MIT \
  --repository https://github.com/codeyousef/SeenLang
```

Do not edit, rebuild, or repack this version after publishing it. Bump the
version before the next registry smoke submission.
