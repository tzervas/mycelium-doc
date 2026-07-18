# CROSS-REF — mycelium-doc

Mycelium-internal dependencies only (steer handoff §6.1; external crates stay in Cargo
metadata). Pinned revs are the fixed (buildable) tips recorded by the Phase-B wave;
content hash = git tree hash of the pinned rev.

| Interface consumed | Repo | Pinned rev | Content hash | Notes |
|---|---|---|---|---|
| mycelium-core | https://github.com/tzervas/mycelium-core | `46d2515cbd86d2ae4d1365f4adcd2796737e9f0b` | tree `(tree hash: fetch dep rev locally to resolve)` | Rust API of `mycelium-core` (see monorepo `docs/api-index/INDEX.md#mycelium-core`) |
| mycelium-l1 | https://github.com/tzervas/mycelium-l1 | `2b92f54349eb0d4f67e32e983874df76908b9ab6` | tree `(tree hash: fetch dep rev locally to resolve)` | Rust API of `mycelium-l1` (see monorepo `docs/api-index/INDEX.md#mycelium-l1`) |
| mycelium-proj | https://github.com/tzervas/mycelium-proj | `20b8a6d264ac728e81cfe8cd90cec8d2a91370be` | tree `(tree hash: fetch dep rev locally to resolve)` | Rust API of `mycelium-proj` (see monorepo `docs/api-index/INDEX.md#mycelium-proj`) |
| mycelium-workstack | https://github.com/tzervas/mycelium-core | `46d2515cbd86d2ae4d1365f4adcd2796737e9f0b` | tree `(tree hash: fetch dep rev locally to resolve)` | Rust API of `mycelium-workstack` (see monorepo `docs/api-index/INDEX.md#mycelium-workstack`) |

**Owning docs:** see monorepo `docs/Doc-Index.md`.
**Source provenance:** extracted from `tzervas/mycelium` archive `aad96b7a…`; fixed by
the course-correction Phase B (workspace root, git pins, toolchain + supply-chain
replicas, CI v2). Full program record: monorepo
`docs/planning/course-correction-2026-07-18/PROGRAM.md`.
