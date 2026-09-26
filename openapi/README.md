# OpenAPI source

`openapi.yaml` is synced from the Trustify `v0.6.2` release at commit
`b9d2627f83d189f0e7447b6bc0820f95bd061749`.

SHA-256: `b223b565cf7ad271918d66702e41df79c09d48002e35a6d16eea398f3ca9a185`

To update it to another Trustify commit or tag, run:

```sh
scripts/sync-openapi.sh <git-ref>
```

The upstream specification is OpenAPI 3.1.0. The Rust `xtask` normalizes the
3.1 nullable-schema syntax and a few Trustify-spec input/media-type issues for
Progenitor 0.15.0. It keeps the canonical checked-in OpenAPI file unchanged;
the request hook restores the merge-patch content type at runtime. Run CI (or
`scripts/generate-rust.sh`) whenever this source or the generator changes.
