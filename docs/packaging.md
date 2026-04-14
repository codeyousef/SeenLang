# Seen Packaging

Seen packages are source archives served from a static registry. A registry can
live on a normal website or object storage bucket as long as it serves files
over HTTPS.

The current default public registry URL is:

```toml
[registries]
default = "https://seen.yousef.codes/packages"
```

## Package Manifest

Packages use the same `Seen.toml` format as applications:

```toml
[project]
name = "mathx"
version = "0.1.0"
language = "en"

[dependencies]

[native.dependencies]
```

The package name is currently also the dependency key and import root.

```seen
import mathx.value.{answer}
```

## Consumer Setup

```toml
[project]
name = "demo"
version = "0.1.0"

[registries]
default = "https://seen.yousef.codes/packages"

[dependencies]
mathx = "0.1.0"

[native.dependencies]
```

Registry dependencies are fetched into `.seen/packages/`, and registry-backed
projects get a `Seen.lock`.

## Static Registry Layout

The compiler expects this layout under the registry base URL:

```text
packages/
├── index/
│   └── mathx.toml
└── archives/
    └── mathx/
        └── mathx-0.1.0.seenpkg.tgz
```

Each package index is a TOML file:

```toml
version = 1

[[releases]]
version = "0.1.0"
archive = "archives/mathx/mathx-0.1.0.seenpkg.tgz"
sha256 = "..."
```

## Publishing

`seen pkg publish` writes into a local static registry directory:

```bash
seen pkg publish ./dist/registry ./path/to/package
```

For convenience, this repo also includes:

```bash
scripts/publish_registry.sh --manifest ./path/to/package
```

That script publishes into `dist/registry` by default. If you also pass
`--sync-dir`, it mirrors the registry tree into your website checkout or deploy
directory:

```bash
scripts/publish_registry.sh \
  --manifest ./examples/mathx \
  --sync-dir /path/to/site/public/packages
```

If your hosting setup serves `/path/to/site/public/packages` at
`https://seen.yousef.codes/packages`, the package is ready for consumers after
you deploy the site.

## Deploying To `seen.yousef.codes`

One simple flow is:

1. Build a compiler with `pkg` support.
2. Run `scripts/publish_registry.sh --manifest <package> --sync-dir <site-packages-dir>`.
3. Deploy your site as usual.
4. Verify that `https://seen.yousef.codes/packages/index/<package>.toml` is reachable.
5. Verify that a separate Seen project can `seen pkg fetch` from the hosted URL.

## Current Limits

- Registry versions are exact-only for now. `^`, `~`, and `*` are rejected.
- `seen pkg publish` writes to local directories; it does not upload over HTTP.
- Package name, dependency key, and import root are the same in this MVP.
- Packages are source archives only; there are no prebuilt binaries yet.

## Related

- [Project Configuration](project-config.md)
- [CLI Reference](cli-reference.md)
