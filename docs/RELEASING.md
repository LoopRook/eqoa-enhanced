# Versioned Enhanced releases

Players download one Windows ZIP and double-click eqoa.exe. Release builds follow
https://github.com/LoopRook/eqoa-enhanced/releases/latest/download/; plain local
builds have no channel. Never point enhanced builds at the upstream vanilla release.

## Publish

1. Merge reviewed upstream work into enhanced, preserving fork features. Run the
   footprint checker, relevant tests and native/browser builds; record fixture skips.
2. Bump VERSION using MAJOR.MINOR.PATCH. Update the player download link and add
   docs/releases/<version>.md plus FORK-UPDATE-NOTES.md entries. Use patch increments
   for fixes and minor increments for substantial additions; major0 is early development.
3. Commit and push tested enhanced. Create and push the matching immutable v<version>
   tag. The Enhanced versioned release workflow builds Windows x64, runs tests,
   stamps channel enhanced/exact commit, and packages only the allowed runtime files.
4. CI verifies the source, PE x64 GUI/8MiB stack, embedded commit/fork URL, catalog,
   complete ZIP, hashes and update.txt before publishing. A draft becomes latest
   only after all assets are uploaded. Published versions cannot be overwritten;
   subsequent versions must increase numerically. A failed draft can be retried.
5. Check the public ZIP download, checksums, update.txt and notes. Test direct EXE
   launch/catalog loading and the local updater fixture. Record CI/source hashes.

## Assets

- EQOA-Enhanced-<version>-Windows-x64.zip: player download, with eqoa.exe, catalog,
  required license notices, short README and provenance manifest. No BAT, source,
  developer tools, profile/account files or game disc data.
- eqoa-enhanced-windows-x86_64.exe: raw updater payload, identical to the ZIP's EXE.
- update.txt: semantic version, source commit, payload SHA256 and byte length.
- update-notes.md: FORK-UPDATE-NOTES.md, matching the notes compiled into this build.
- SHA256SUMS: checksums for the four assets above.

The platform currently published by this workflow is Windows x64. Other platforms
still build from source but need their own verified runtime packages before being
added to the updater manifest. Upstream master/dev workflows remain unchanged and
must not be used to publish this fork. The companion parser is optional/separate.
Existing license terms/notices remain unchanged; no alternative license was chosen.
