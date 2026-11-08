# Private builds (retired 2026-11-07)

From 0.4.4 to 0.4.10, official player builds were made in a separate private
repository, `LoopRook/eqoa-enhanced-private`, and uploaded to this
repository's releases. That repository held the live shard compatibility code
and its own `vX.Y.Z` tags, which share names with this repository's tags but
name different commits. Those old tags are left as they are.

On 2026-11-07 the compatibility code became public source
([LIVE-CLIENT-COMPATIBILITY.md](LIVE-CLIENT-COMPATIBILITY.md)) and the
publisher moved here (`.github/workflows/official-release.yml`,
`tools/official_release.py`). From 0.4.11 on, version tags exist only in this
repository; see [RELEASING.md](RELEASING.md). The private repository is
archived read-only once 0.4.11's download and auto-update are confirmed.

The update channel did not change: players still follow
`releases/latest/download/update.txt`, and the updater compares commit IDs, so
a 0.4.10 build (private commit) updates to 0.4.11 (public commit) normally.

The former private runbook is in this file's history before this change.
