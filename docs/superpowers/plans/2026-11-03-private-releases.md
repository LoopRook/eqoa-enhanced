# Private builds and public releases implementation plan

**Goal:** Build official clients from the complete private fork and publish only reviewed runtime assets to the public release channel.

**Architecture:** Public source CI has read access and publishes no binaries. A private, tag-pinned publisher validates a pinned public source tag, builds the full private tree, then verifies all assets before publishing a public draft as latest. Public and private tags share a semantic version but identify different commits.

**Spec:** Owner's November 3 request; operational design and runbook in [PRIVATE-BUILDS.md](../../PRIVATE-BUILDS.md).

## Constraints

- No remote writes, GitHub configuration changes, tags, or releases during implementation.
- Public-to-private sync uses merges; exports use reviewed single-commit cherry-picks, never private branch merges.
- Compatibility implementation and local private notes never enter public source or public CI.
- Keep the existing Enhanced download URL, asset names, manifest format, notices, and updater implementation.

## Tasks

- [ ] Export the defaults onto a public feature branch from develop. Sanitize release notes before committing. Verify the private change is absent from its ancestry and tree.
- [ ] Replace the public versioned publisher with read-only Windows build/test CI. Gate every legacy upstream publishing job. Add negative-control workflow checks.
- [ ] Merge the public feature into an isolated private feature branch; retain all private code. Add a private public-source pin, private publisher workflow, preflight/export guards, publication tests, and private-only policy ownership.
- [ ] Test guards against wrong tags, changed pins, published-release retries, missing/unexpected assets, stale versions, and changed tags. Run public and private Rust build/test suites serially, release tooling tests, and footprint checks. Record prerequisite skips.
- [ ] Document credential setup, activation order, exact push/tag/run commands, verification, rollback, and a test matrix. Obtain independent read-only review. Leave both feature branches local for owner review.

## Review focus

- Old public workflows or old tag workflows can still publish before migration is activated.
- GitHub concurrency groups apply within one repository; an external writer is outside the lock.
- A public source tag must never point at a private commit, including through merged history.
- The updater compares commit identity, so one authoritative manifest and matching embedded private SHA are essential.
- Draft retries must not replace a different build, and published assets must never be overwritten.
