# Supply-chain export

Conduit publishes the accepted x86_64 ConduitOS image as a non-container OCI
image layout alongside its ordinary release files. The layout is distribution
plumbing: reading or copying it does not install, boot, admit, plan, or execute
the image.

The root OCI manifest retains the existing Conduit `ArtifactId` and `BuildId`
as annotations and addresses the image layer by its exact SHA-256 digest. Three
separately typed OCI referrers point back to that root manifest:

- the existing Conduit build manifest;
- an in-toto Statement v1 carrying SLSA provenance v1 fields known from the build;
- a Conduit digest-verification receipt whose `source-capability` proof class
  explicitly does not claim installation, boot, physical execution, membership,
  or authority.

The export makes no SLSA level claim. Its provenance records the source commit,
builder adapter, invocation, BuildId, ArtifactId, and output digest; it does not
translate Host, Boot, Body, Plan, Play, or physical-proof facts into SLSA.

CI creates the layout from the already sealed release, reads it back through a
fresh digest-verifying consumer, and carries it into Pages at
`supply-chain/conduitos-x86_64-pc/`. Current product truth records the layout's
index digest and exact root-manifest digest. A mutable registry tag or index
annotation can help humans find the layout, but cannot redefine either Conduit
identity.

Registry or network failure is only distribution failure. Verification answers
whether bytes and statements match their digests; separate policy decides
whether a builder is accepted, a proof class is sufficient, an image is
compatible with a Host, or an operation is authorized for a Body.

Repository and CI development use the internal bounded exporter directly:

```text
node tools/ci/oci-release-export.mjs prepare-conduitos RELEASE BUILD_MANIFEST OUTPUT_PREFIX SOURCE_SHA INVOCATION_ID
node tools/ci/oci-release-export.mjs export OUTPUT_PREFIX.config.json OCI_LAYOUT
node tools/ci/oci-release-export.mjs verify OCI_LAYOUT
```

This entrance deliberately writes a local OCI layout. A later registry push may
transport the same blobs and referrer relationships without changing identity
or introducing a container runtime.
