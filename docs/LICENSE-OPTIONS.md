# License options — decision pending

## Current position

LICENSE already contains **MIT, copyright 2026 DabDavis**, and Cargo.toml declares
MIT. This integration leaves both unchanged. The README describes the enhanced
fork; it does not replace or remove upstream attribution.

A choice can govern contributions you have authority to license and the fork's
distribution on permitted terms. It does not erase upstream MIT notices or the
rights already granted to existing recipients. Keep Space Mono's OFL notice and
the Forge catalog's MIT notice separately. A client-code license does not grant
rights to distribute the game's disc data.

## Choices

| Choice | What recipients can do / must share | Main trade-off |
|---|---|---|
| **MIT** | Broad reuse, modification and distribution, including proprietary reuse; retain the copyright/license notice. | Short and aligned with upstream. No requirement to publish modifications. |
| **Apache-2.0** | Broad reuse with an explicit contributor patent grant; retain required notices and identify modified files. | More detailed protection and notice obligations; modifications need not be published. |
| **MPL-2.0** | File-level copyleft: distributing covered executables requires availability of covered source, including modifications to MPL files. Separate files can retain other terms. | A middle ground when you want changes to covered files shared without requiring the entire larger work to use MPL. |
| **GPL-3.0** | Distributing a covered modified/combined program carries GPL source and license obligations for that covered work. | Stronger reciprocal sharing, with more constraints on combining/distributing code. Private use alone does not require publication. |

These summaries describe the main distinctions, not every license condition.
Before adopting different terms, inventory dependencies and existing contribution
permissions, and update notices and package metadata consistently.

## How to decide

- Prefer easy reuse and matching upstream contribution terms: consider **MIT**.
- Prefer permissive reuse with explicit patent terms: consider **Apache-2.0**.
- Want distributed modifications to covered files shared: consider **MPL-2.0**.
- Want the distributed covered combined program kept under reciprocal terms:
  consider **GPL-3.0**.

A different license can complicate contributing fork changes back to an MIT-only
upstream; contributor-approved dual licensing is a separate option, not something
applied here. The owner will choose. No alternate license file, source header or
Cargo license field has been applied.

## Primary sources checked October 28, 2026

- [MIT text — Open Source Initiative](https://opensource.org/license/mit)
- [Apache License 2.0 — Apache Software Foundation](https://www.apache.org/licenses/LICENSE-2.0)
- [MPL 2.0 FAQ — Mozilla](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)
- [GPL 3.0 text — Open Source Initiative / FSF](https://opensource.org/license/gpl-3.0)
