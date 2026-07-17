# Pitgun CLI Release Process

Pitgun CLI releases are immutable builds tied to an annotated Git tag and the
commit selected by that tag. The first distributed version is
`v0.1.0-alpha.1`; crates.io publication remains out of scope.

## Before tagging

1. Merge the release-preparation pull request into `main`.
2. Confirm `build`, `racing-e2e`, and `wasm-golden-run` are green.
3. Confirm the release workflow built all configured target archives on the
   pull request.
4. Confirm the `pitgun-cli` package version is the tag without its leading `v`.
5. Run the workspace and packaged-binary quickstarts on at least one supported
   native target.

Never move or reuse a published release tag. Correct a faulty release with a
new pre-release or patch version.

## Publish

From an up-to-date `main` branch:

```bash
git tag -a v0.1.0-alpha.1 -m "Pitgun CLI v0.1.0-alpha.1"
git push origin v0.1.0-alpha.1
```

The `release-cli` workflow validates that the tag matches the Cargo package
version, builds the macOS and Linux archives, generates `SHA256SUMS`, verifies
the archives, and publishes a GitHub pre-release. No release should be created
manually before that workflow completes.

## Validate the published release

On a clean supported machine:

1. Follow [the public quickstart](QUICKSTART.md), including checksum validation.
2. Confirm `pitgun --version` reports the tagged version.
3. Confirm the seed-42 demo reaches the published `VERIFIED` run identity in
   less than five minutes.
4. Confirm replaying the generated bundle succeeds in a fresh process.
5. Record the measured target and duration in the GitHub Release notes or the
   release-tracking issue.

Only then is the distribution ticket complete.
