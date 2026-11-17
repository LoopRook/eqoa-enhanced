# Modding EQOA Enhanced

EQOA Enhanced runs native Lua mods through the `eqoa` API (`eqoa/1`). Start here.

| Page | Read it for |
|---|---|
| [GETTING-STARTED.md](GETTING-STARTED.md) | The first mod, step by step, and the common patterns |
| [template/](template/) | A starter folder (`mod.json` + `main.lua`) to copy |
| [EXAMPLES.md](EXAMPLES.md) | What each bundled mod shows and which API areas it uses |
| [EQOA-API-SPEC.md](EQOA-API-SPEC.md) | The exact contracts: every function, record, event, error |
| [VERSIONING.md](VERSIONING.md) | What `eqoa/1` promises, and how to test for features |
| [AGENT-GUIDE.md](AGENT-GUIDE.md) | One page for an AI agent building a mod from a brief |

Status: the spec is still being brought up to date with the code (see the round 7 plan,
`docs/plans/eqoa-api/`). Where a page and the running client disagree, the client's own
answer wins: ask it with `eqoa.capabilities.get(path)`.
