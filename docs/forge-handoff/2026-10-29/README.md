# Preserved continuation material

Read ../../FORGE-COMPATIBILITY-HANDOFF.md first.

These copies preserve ignored target-directory work so a new agent does not lose
it when build caches disappear. Text was normalized to UTF-8/LF and trailing
whitespace removed; manifest.json records both original and saved hashes.

- sound.rs.txt is an UNCOMPILED implementation draft, not active Rust. Its tests
  have not run, and its native/lifecycle behavior is not established. Do not
  copy it into the crate without review, tests and integration.
- sound-contract-next.md supersedes the early audio.rs mixer proposal still
  described in parts of media-contract.md. Current choice is Windows MCI,
  matching UiForge's native implementation; other platforms remain unsupported.
- media-contract.md's corrected font section is design only. Prefer the safe
  StbTrueTypeFontData path and queued pre-frame atlas mutation. Absolute claims
  such as "100% memory safety" in this draft are not verification evidence.
- uiforge-next-gap.md is a historical audit BEFORE the common widget slice and
  stdlib landed. Many listed gaps are now implemented; compare actual source.
- player-next-contract.md is a source audit, NOT reviewed implementation approval.
  Target sentinel, walker access and snapshot shape still need resolution.
- prepare-preview.ps1.txt was executed successfully for the early preview after
  root corrections. Its older packaging notes may still say 15 files; the actual
  final archive contains 16, including Native-Addon-Runtime-NOTICES.txt.

No source-reference scripts, profiles, disc images, credentials, or texture packs
are copied here. Scripts ending .txt are inert reference material.
