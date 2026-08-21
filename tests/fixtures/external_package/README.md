# Canonical external Seen package fixture

This fixture is a reusable source-only package with the canonical
`seen-package-layout-v1` tree. The sibling fixture at
`../pkg-layout-001/external-consumer/` proves that a package alias can import
the public root from outside the package.

The canonical roots are `Seen.toml`, `src/mod.seen`, `tests/`,
`examples/`, `README.md`, and `LICENSE`. Package archives include only
the declared source, example, readme, and license files.
