# Pitgun public schemas

This directory is the canonical source for the versioned files published at
<https://schemas.pitgun.io>.

Each schema identifier must equal its public URL. Once an active version has
been published, its bytes are immutable; incompatible changes require a new
versioned path. `index.json` records whether a schema is active or experimental
and names its owning component.

The OVH shared host is only the publication layer. Changes originate here,
pass schema and example validation in CI, and are deployed from `main`.
