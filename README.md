# Cura

Cura is where code, read from any language, is kept in one notation.

- **YA|RA** is the notation: every statement written as `intent · weave · pattern`
  (input · logic · output), built from five operations that every language shares —
  read, change, branch, write, repeat.
- **Cura** is the registry that holds what was read: the wire types, the binary
  encoding of each intent, and storage with first-seen, last-seen and counts.

Readers fill it. The first reader is [curator](https://github.com/yashrajvansh/curator),
which recognizes the five operations at the byte level in C-family languages, Python,
Fortran, COBOL, Pascal, Chinese-keyword source and six machine and bytecode forms.
Anyone can write another reader against `cura-protocol` and submit to the same registry.

Why: the world runs on trillions of lines in thousands of languages. No person or
model reads them one language at a time. One shared notation, open to anyone, lets
any of it be read, compared and checked in one place.

## Crates

| Crate | Role |
|---|---|
| `cura-protocol` | YA\|RA wire types: `Op`, `Sheathed`, the `Sheath` trait readers implement, `Triple`, `Plane`, `Origin` |
| `cura-crypto` | HMAC-SHA-256, SHA-256, BLAKE3 |
| `cura-registry` | Storage: the 12-kind binary encoder (`binary.rs`), `PlaneRow`, sinks |
| `cura-worker` | The registry's HTTP surface (Cloudflare Worker, D1) |
| `cura-tenant-stub` | Per-tenant stub deployed into the dispatch namespace |
| `ya-ra` | The YA\|RA language: lexer, parser, evaluator |

Editor support and the Linguist entry for `.YA-RA` files live in
[comfortcurators/YA-RA](https://github.com/comfortcurators/YA-RA).

## Live endpoints (`cura.rajvansh.workers.dev`)

| Route | Auth | Notes |
|---|---|---|
| `GET /health` | none | |
| `GET /v1/weaves` | none | |
| `GET /v1/signatures` | none | |
| `GET /v1/langs` | none | |
| `GET /v1/intents/:v` | none | |
| `GET /v1/patterns/:v` | none | |
| `GET /v1/frontier/:v` | none | |
| `GET /v1/tenants/:id/health` | none | proxies a Dispatch Namespace tenant's own `/health` — not added this session, predates it |
| `POST /v1/weaves` | HMAC (`CURA_ORIGIN_SECRET`) | the real ingest path; writes intent, pattern and weave together, always |
| `POST /v1/healthcheck` | HMAC (`CURA_ORIGIN_SECRET`) | manual trigger for the sink round-trip the cron already runs every 6h |

Read from `crates/cura-worker/src/lib.rs`'s `fetch()` match arm directly, not from this table, before trusting a route you haven't called — this table is documentation and can drift the way `handleRoot`-style endpoint maps do everywhere else in this org.

## Build

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Deploy the worker by hand from `crates/cura-worker`: `wrangler deploy`.

## License

Apache-2.0. Copyright © 2026 Comfort Curators Private Limited. See `LICENSE` and `NOTICE`.
Readers are licensed separately; curator is AGPL-3.0-only.
