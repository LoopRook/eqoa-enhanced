# Accepted sound-stage checkpoint

Code: 4e4dc737ca1c31e9d85a152da2cb48500038db5f, after reviewed substantive
fix 6cab6c1b26eeef10a5e8983d3588bbd7a8dfd530. Local forge-compatibility only;
no public release, push or enhanced merge. Independent MiniMax review accepted
all blockers and the final UTF-8 follow-up. See review.md final rev.5 section.

The original brief and all review rounds are preserved here; earlier findings
in review.md are historical and must be read with their later dispositions.
The original scratch artifacts/logs remain in original-checkout
run/forge-sound-stage/. evidence.json records exact log hashes and local paths.
No game data, profile or account files are included.

Substantive fix: 133 addon tests passed, four existing Windows1314 symlink
privilege skips, no native MCI/GPU/source/PNG skips. Required native test passed
with real open/play/query-playing/volume/stop/query-stopped/close calls. Native
build and WASM check passed on that source. Final Unicode-only follow-up: 23
focused sound tests passed. Do not claim a 134-test full-suite run: it was not
run. Containment and Unicode diagnostic regressions have actual failing-before /
passing-after logs. Initial and negative-ID historical RED was not preserved.

Linux was not executed: Windows-specific tests are gated and the unsupported
platform path is covered in source/tests, reviewed by inspection. MCI success
is not audible acceptance; user listening remains manual. OS alias release
cannot be guaranteed when MCI itself rejects close. Numeric handles are local
to isolated VMs, and resolver duplication remains a documented adaptation.

Fonts and the remaining native game APIs/full unchanged-addon acceptance are
still pending. Existing early preview is older source7bf02d3 and does not contain
sound or later UI/stdlib/drawing changes. No replacement preview was packaged.
