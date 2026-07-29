# Catalog Publication

Pitgun publishes validated Resource Catalog releases as static files at
[`catalog.pitgun.io`](https://catalog.pitgun.io). No PHP application, database,
or VPS process is involved.

## Public layout

```text
catalog.pitgun.io/
└── racing/
    ├── latest.json
    └── v1.0.0/
        ├── catalog.json
        ├── release.json
        ├── simulation/
        └── presentation/
```

Every `vX.Y.Z` directory is immutable. Its files receive a one-year
`Cache-Control` lifetime with the `immutable` directive. Public CORS permits
browser, CLI, authority, and verifier consumers to retrieve the same bytes.

`latest.json` is the only mutable catalog artifact. It receives `no-cache` and
contains:

- the selected catalog ID and exact version;
- the immutable manifest path;
- the manifest's canonical SHA-256 digest;
- the immutable release-identity path.

Consumers may use it for discovery, but an execution must pin the immutable
version and identities it resolves.

## GitHub environment

The deployment job uses the protected `catalog-production` environment with
these environment secrets:

- `SFTP_HOST`;
- `SFTP_PORT`;
- `SFTP_USER`;
- `SFTP_PASSWORD`;
- `SFTP_PATH`, set to `/home/loicbelehy/pitgun/catalog`.

The OVH multisite document root for `catalog.pitgun.io` must be that same
directory.

## Publishing a release

1. Add a new complete release under `catalogs/<domain>/vX.Y.Z`.
2. Generate and validate all indexes and identities.
3. Leave `catalogs/<domain>/LATEST` unchanged while the release is being
   reviewed.
4. Change `LATEST` to the new stable semantic version when it is ready for
   discovery.
5. Merge through `main`.

The deployment workflow then:

1. validates schemas, resource bytes, indexes, pack identities, and release
   identities;
2. builds a static publication tree;
3. checks every already-public version against the local source;
4. refuses publication if one historical byte differs;
5. uploads each new version to a unique temporary directory;
6. renames the temporary directory atomically to its final version;
7. updates `latest.json`;
8. downloads every public file and compares its exact bytes;
9. validates the public pointer, digest, CORS, MIME type, and cache headers.

The workflow never mirrors with `--delete` and never uploads into an existing
version directory.

## Idempotent deployment

Re-running the workflow is safe:

- an existing identical release is not uploaded again;
- a missing release is published;
- an existing divergent release fails the job;
- `latest.json` is regenerated from the selected `LATEST` marker.

## Rollback

A catalog rollback does not delete or mutate a release.

1. Change `catalogs/<domain>/LATEST` to an older version that remains checked
   in and publicly available.
2. Merge the change through `main`.
3. Verify that the deployment job updated `latest.json`.

This only changes future discovery. Contracts and runs already pinned to a
newer immutable release remain reproducible and continue resolving that release.

If publication fails before the atomic rename, the final version URL remains
absent. A hidden temporary upload directory may remain on OVH and can be removed
after investigation; it is never referenced by `latest.json`.
