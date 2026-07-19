# Pitgun public schemas

This directory is the canonical source for the versioned files published at
<https://schemas.pitgun.io>.

`index.json` distinguishes JSON Schemas from versioned data dictionaries. Each
artifact identifier must equal its public URL. Once a version has been
published, its bytes are immutable; incompatible changes require a new
versioned path.

Lifecycle states have precise meanings:

- `active`: a current producer or consumer exists and is named as executable
  evidence in the catalog;
- `experimental`: implementation work is current, but the contract is not yet
  a supported integration boundary;
- `legacy`: the URL remains available for reference, but no current platform
  capability should depend on it.

The historical `bundle-manifest/v1` describes a prototype registry bundle. It
is unrelated to the deterministic Run Bundle contract implemented by
`pitgun-contract`.

The OVH shared host is only the publication layer. Changes originate here,
pass schema and example validation in CI, and are deployed from `main`.
