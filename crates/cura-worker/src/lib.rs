//! Cura — the registry. Serves YA|RA over HTTP.

pub mod d1_sink;
pub mod healthcheck;

use cura_protocol::Plane;
use worker::*;

/// A quiet self-check, not a monitor. Runs `healthcheck::run` --
/// writes a marker fact through the real `D1Sink` (the same path
/// every real curator translate run uses, not a raw-SQL shortcut
/// around it), reads it back, deletes it from all three planes.
///
/// `scheduled()` itself is awaited by the runtime to completion,
/// unlike a `fetch` handler which returns a response before
/// background work is done -- so this awaits the healthcheck
/// directly, no `wait_until` needed. That's exactly the distinction
/// the wait_until fix (2026-09-15) turned on, so it's worth keeping
/// straight here: this function has nothing to race against.
///
/// Logs only. `console_log!`/`console_error!` go to Workers Logs and
/// nowhere else -- no notification, no external call. If it ever
/// needs to bother anyone, that's a decision for later, made on
/// purpose, not a default this function reaches for.
///
/// Replaces the earlier raw-SQL canary (direct INSERT into `weave`
/// alone): that version never exercised `D1Sink` -- the thing every
/// real write actually goes through -- and never cleaned up after
/// itself, so `smoketest_canary`'s occurrence count grew without
/// bound on every cron tick.
#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    if let Err(e) = crate::healthcheck::run(&env).await {
        console_error!("cura healthcheck: {e}");
    }
}

#[event(fetch)]
pub async fn fetch(req: Request, env: Env, ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path().to_string();
    let method = req.method();

    match (method, path.as_str()) {
        (Method::Get, "/health") => health(),
        (Method::Get, "/v1/weaves") => list_weaves(req, env).await,
        (Method::Get, "/v1/signatures") => signatures(req, env).await,
        (Method::Get, "/v1/langs") => langs_by_intent(req, env).await,
        (Method::Get, p) if p.starts_with("/v1/intents/") => {
            let v = p.trim_start_matches("/v1/intents/");
            resolve_by_intent(req, env, v).await
        }
        (Method::Get, p) if p.starts_with("/v1/patterns/") => {
            let v = p.trim_start_matches("/v1/patterns/");
            resolve_by_pattern(req, env, v).await
        }
        (Method::Get, p) if p.starts_with("/v1/frontier/") => {
            let v = p.trim_start_matches("/v1/frontier/");
            frontier(req, env, v).await
        }
        (Method::Post, "/v1/weaves") => post_weave(req, env, ctx).await,
        (Method::Post, "/v1/healthcheck") => trigger_healthcheck(req, env).await,
        (Method::Get, p) if p.starts_with("/v1/tenants/") && p.ends_with("/health") => {
            tenant_health(p, env).await
        }
        _ => Response::error("not found", 404),
    }
}

fn health() -> Result<Response> {
    Response::from_json(&serde_json::json!({
        "status": "ok",
        "registry": "cura",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Curator's job isn't scoped to one enterprise. A dispatch namespace lets
/// cura route by name into a per-target Cura instance -- one script per
/// mirrored repo/enterprise, uploaded into the namespace, no separate
/// deploy or DNS per tenant.
async fn tenant_health(path: &str, env: Env) -> Result<Response> {
    let name = path
        .trim_start_matches("/v1/tenants/")
        .trim_end_matches("/health")
        .to_string();
    if name.is_empty() {
        return Response::error("missing tenant name", 400);
    }

    let dispatcher = env.dynamic_dispatcher("TENANTS")?;
    let fetcher = match dispatcher.get(name.clone()) {
        Ok(f) => f,
        Err(e) => return Response::error(format!("unknown tenant `{name}`: {e}"), 404),
    };

    let resp = fetcher
        .fetch("https://tenant/health".to_string(), None)
        .await?;
    let status = resp.status_code();
    let mut resp = resp;
    let body_text = resp.text().await.unwrap_or_default();
    let tenant_body: serde_json::Value =
        serde_json::from_str(&body_text).unwrap_or(serde_json::json!({"raw": body_text}));

    Response::from_json(&serde_json::json!({
        "tenant": name,
        "status_code": status,
        "response": tenant_body,
    }))
}

/// Kept for the intent/pattern planes even though none of the current
/// read handlers call it — every one of them queries DB_DIP directly,
/// since the two-axis views (v_intent_frontier, v_signature_patterns,
/// v_language_by_intent) are DIP-local by design. If a read handler
/// for the DI or DP plane is ever added, this is the dispatch it needs;
/// deleting it now would mean re-deriving the same three-way match.
#[allow(dead_code)]
fn plane_from_query(url: &Url) -> Plane {
    for (k, v) in url.query_pairs() {
        if k == "plane" {
            return match v.as_ref() {
                "intent" => Plane::Intent,
                "pattern" => Plane::Pattern,
                _ => Plane::Weave,
            };
        }
    }
    Plane::Weave
}

#[allow(dead_code)]
fn binding_for(p: Plane) -> &'static str {
    match p {
        Plane::Intent => "DB_DI",
        Plane::Weave => "DB_DIP",
        Plane::Pattern => "DB_DP",
    }
}

/// R2 body storage was dropped along with the old weave_id addressing
/// (see post_weave's doc comment) — no live handler calls this. Kept
/// for the same reason as plane_from_query: restoring body storage
/// against the new composite key needs this same bucket mapping.
#[allow(dead_code)]
fn bucket_for(p: Plane) -> &'static str {
    match p {
        Plane::Intent => "BODIES_RI",
        Plane::Weave => "BODIES_RIP",
        Plane::Pattern => "BODIES_RP",
    }
}

#[allow(dead_code)]
fn table_for(p: Plane) -> &'static str {
    p.as_str()
}

/// One row from the `weave` table (DB_DIP), the shape every read
/// handler below shares. `witness` is truncated to 500 chars on the
/// way out (see `FactResponse::from`) — it can be a whole file's worth
/// of source text and nothing here needs more than a preview of it.
#[derive(serde::Deserialize)]
struct WeaveFact {
    weave: String,
    pattern: String,
    intent: String,
    op: String,
    source_lang: String,
    witness: String,
    #[allow(dead_code)]
    observed_at: i64,
    occurrences: i64,
    first_seen: i64,
    last_seen: i64,
}

#[derive(serde::Serialize)]
struct FactResponse {
    weave: String,
    pattern: String,
    intent: String,
    op: String,
    source_lang: String,
    witness: String,
    occurrences: i64,
    first_seen: i64,
    last_seen: i64,
}

impl From<WeaveFact> for FactResponse {
    fn from(f: WeaveFact) -> Self {
        // Byte-slicing at a fixed offset can land mid-codepoint and
        // panic on multi-byte UTF-8 (e.g. a CJK-heavy witness) --
        // char_indices finds the nearest valid boundary at or before
        // 500 instead of assuming byte 500 is one.
        let witness = if f.witness.len() > 500 {
            let cut = f
                .witness
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= 500)
                .last()
                .unwrap_or(0);
            format!("{}…", &f.witness[..cut])
        } else {
            f.witness
        };
        FactResponse {
            weave: f.weave,
            pattern: f.pattern,
            intent: f.intent,
            op: f.op,
            source_lang: f.source_lang,
            witness,
            occurrences: f.occurrences,
            first_seen: f.first_seen,
            last_seen: f.last_seen,
        }
    }
}

const FACT_COLS: &str = "weave, pattern, intent, op, source_lang, witness, \
     observed_at, occurrences, first_seen, last_seen";

/// Facts in last-seen order, most recent first. `before` (a
/// millisecond timestamp) pages backward through that ordering.
///
/// There is no `/v1/weaves/:id` anymore — the new schema's identity
/// is the composite `(weave, pattern, intent)`, not a single opaque
/// id. See `/v1/intents/:value` and `/v1/patterns/:pattern` for
/// addressed lookups, and STATE.md for why the old shape is gone.
async fn list_weaves(req: Request, env: Env) -> Result<Response> {
    let url = req.url()?;
    let mut limit: usize = 20;
    let mut before: Option<i64> = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "limit" => limit = v.parse().unwrap_or(20).min(200),
            "before" => before = v.parse().ok(),
            _ => {}
        }
    }

    let db = env.d1("DB_DIP")?;

    let sql = if before.is_some() {
        format!(
            "SELECT {FACT_COLS} FROM weave WHERE last_seen < ?1 \
             ORDER BY last_seen DESC LIMIT ?2"
        )
    } else {
        format!("SELECT {FACT_COLS} FROM weave ORDER BY last_seen DESC LIMIT ?1")
    };

    let stmt = db.prepare(&sql);
    let bound = if let Some(t) = before {
        stmt.bind(&[t.into(), (limit as f64).into()])?
    } else {
        stmt.bind(&[(limit as f64).into()])?
    };

    let rows: Vec<WeaveFact> = bound.all().await?.results()?;
    let facts: Vec<FactResponse> = rows.into_iter().map(Into::into).collect();

    Response::from_json(&serde_json::json!({
        "count": facts.len(),
        "weaves": facts,
    }))
}

/// Every way a given intent value has been reached — one row per
/// distinct `(pattern, weave)`, ordered by how often each way has
/// actually happened.
async fn resolve_by_intent(_req: Request, env: Env, value: &str) -> Result<Response> {
    if value.is_empty() {
        return Response::error("empty intent", 400);
    }
    let db = env.d1("DB_DIP")?;
    let sql = format!(
        "SELECT {FACT_COLS} FROM weave WHERE intent = ?1 ORDER BY occurrences DESC, last_seen DESC"
    );
    let rows: Vec<WeaveFact> = db
        .prepare(&sql)
        .bind(&[value.into()])?
        .all()
        .await?
        .results()?;

    if rows.is_empty() {
        return Response::error("no facts for that intent", 404);
    }

    let facts: Vec<FactResponse> = rows.into_iter().map(Into::into).collect();
    let binary = cura_registry::binary_of_str(value);

    Response::from_json(&serde_json::json!({
        "intent": value,
        "binary": binary,
        "count": facts.len(),
        "ways": facts,
    }))
}

/// Every fact sharing a given pattern — the surface a pattern is
/// reached from, and what it actually reaches, across every language
/// that has produced it.
async fn resolve_by_pattern(_req: Request, env: Env, pattern: &str) -> Result<Response> {
    if pattern.is_empty() {
        return Response::error("empty pattern", 400);
    }
    let db = env.d1("DB_DIP")?;
    let sql = format!("SELECT {FACT_COLS} FROM weave WHERE pattern = ?1 ORDER BY last_seen DESC");
    let rows: Vec<WeaveFact> = db
        .prepare(&sql)
        .bind(&[pattern.into()])?
        .all()
        .await?
        .results()?;

    if rows.is_empty() {
        return Response::error("no facts with that pattern", 404);
    }

    let facts: Vec<FactResponse> = rows.into_iter().map(Into::into).collect();
    Response::from_json(&serde_json::json!({
        "pattern": pattern,
        "count": facts.len(),
        "facts": facts,
    }))
}

#[derive(serde::Deserialize)]
struct FrontierRow {
    intent: String,
    patterns: i64,
    weaves: i64,
    languages: i64,
    total_occurrences: i64,
    has_singletons: i64,
}

/// `v_intent_frontier`'s summary for one intent: how many distinct
/// patterns/weaves/languages reach it, and whether any of those ways
/// has been observed exactly once.
async fn frontier(_req: Request, env: Env, value: &str) -> Result<Response> {
    if value.is_empty() {
        return Response::error("empty intent", 400);
    }
    let db = env.d1("DB_DIP")?;
    let row: Option<FrontierRow> = db
        .prepare(
            "SELECT intent, patterns, weaves, languages, \
                    total_occurrences, has_singletons \
             FROM v_intent_frontier WHERE intent = ?1",
        )
        .bind(&[value.into()])?
        .first(None)
        .await?;

    match row {
        Some(r) => Response::from_json(&serde_json::json!({
            "intent": r.intent,
            "binary": cura_registry::binary_of_str(value),
            "patterns": r.patterns,
            "weaves": r.weaves,
            "languages": r.languages,
            "total_occurrences": r.total_occurrences,
            "has_singletons": r.has_singletons != 0,
        })),
        None => Response::error("no frontier for that intent", 404),
    }
}

#[derive(serde::Deserialize)]
struct SignatureRow {
    pattern: String,
    source_lang: String,
    weaves: String,
}

/// `v_signature_patterns`: patterns reached from exactly one language.
async fn signatures(_req: Request, env: Env) -> Result<Response> {
    let db = env.d1("DB_DIP")?;
    let rows: Vec<SignatureRow> = db
        .prepare("SELECT pattern, source_lang, weaves FROM v_signature_patterns LIMIT 200")
        .all()
        .await?
        .results()?;

    let sigs: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "pattern": r.pattern,
                "source_lang": r.source_lang,
                "weaves": r.weaves.split(',').collect::<Vec<_>>(),
            })
        })
        .collect();

    Response::from_json(&serde_json::json!({
        "count": sigs.len(),
        "signatures": sigs,
    }))
}

#[derive(serde::Deserialize)]
struct LangIntentRow {
    source_lang: String,
    intent: String,
    patterns: i64,
}

/// `v_language_by_intent`, grouped by language rather than returned as
/// flat rows — one entry per language, each carrying every intent it
/// reaches and how many distinct patterns reach it that way.
async fn langs_by_intent(_req: Request, env: Env) -> Result<Response> {
    let db = env.d1("DB_DIP")?;
    let rows: Vec<LangIntentRow> = db
        .prepare("SELECT source_lang, intent, patterns FROM v_language_by_intent LIMIT 500")
        .all()
        .await?
        .results()?;

    use std::collections::BTreeMap;
    let mut grouped: BTreeMap<String, Vec<(String, i64)>> = BTreeMap::new();
    for r in rows {
        grouped
            .entry(r.source_lang)
            .or_default()
            .push((r.intent, r.patterns));
    }

    let langs: Vec<serde_json::Value> = grouped
        .into_iter()
        .map(|(lang, intents)| {
            serde_json::json!({
                "lang": lang,
                "intent_count": intents.len(),
                "intents": intents.into_iter()
                    .map(|(i, n)| serde_json::json!({"intent": i, "patterns": n}))
                    .collect::<Vec<_>>(),
            })
        })
        .collect();

    Response::from_json(&serde_json::json!({
        "count": langs.len(),
        "languages": langs,
    }))
}

#[derive(serde::Deserialize)]
struct PostBody {
    plane: String,
    input: String,
    logic: String,
    output: String,
    source: String,
    // Accepted for wire compatibility with existing callers; nothing
    // in the new schema reads either field.
    #[serde(default)]
    #[allow(dead_code)]
    sheaf_kind: String,
    #[allow(dead_code)]
    duration_ms: u64,
    #[serde(default)]
    lang: String,
    #[serde(default)]
    op: String,
}

/// Writes one triple into the live DI/DP/DIP schema via `D1Sink`.
///
/// Replaces the old direct-SQL path, which inserted into columns
/// (`weave_id`, `sheaf_kind`, `stalk`, `duration_ms`, `body_key`,
/// `origin`, `created_at`) that stopped existing on the live tables
/// the moment the old Record/Plane schema was dropped and replaced by
/// `schema/cura-{di,dp,dip}.sql`. That drop happened weeks before this
/// fix: this handler had been writing to a schema that no longer
/// existed since then, silently, because nothing had exercised it —
/// every verification of the new schema and D1Sink ran direct SQL
/// against D1, never through this endpoint. This is the same failure
/// this file already documents about other faculties: a path nothing
/// calls is not evidence it works.
///
/// R2 body storage, the KV compute-decay cache, and the Vectorize
/// upsert are dropped along with the old `weave_id`-keyed addressing
/// they depended on — the new schema's identity is the composite
/// `(weave, pattern, intent)`, not a single opaque id. If any of the
/// three is wanted again, it needs a real design against the new key
/// shape, not a patch of the old one.
///
/// `sheaf_kind` is accepted but currently unused — nothing in the new
/// schema reads it. Kept on the wire so existing callers don't need to
/// stop sending it.
/// Manual trigger for the scheduled healthcheck, gated behind the same
/// origin-secret HMAC scheme as `post_weave` -- no body, so the
/// canonical string hashes an empty byte slice, matching how a caller
/// with no payload signs it. Runs `healthcheck::run` synchronously and
/// reports its real outcome, so the sink/D1 round-trip can be verified
/// on demand instead of waiting up to 6 hours for the next cron tick.
async fn trigger_healthcheck(req: Request, env: Env) -> Result<Response> {
    let signature = match req.headers().get("x-cura-origin-signature")? {
        Some(s) => s,
        None => return Response::error("missing signature", 401),
    };

    let ts_header = req
        .headers()
        .get("x-cura-origin-timestamp")?
        .ok_or_else(|| Error::RustError("missing timestamp".into()))?;
    let timestamp: i64 = ts_header
        .parse()
        .map_err(|_| Error::RustError("invalid timestamp".into()))?;

    let now = (js_sys::Date::now()) as i64;
    if (now - timestamp).abs() > 300_000 {
        return Response::error("stale request", 401);
    }

    let secret = env
        .secret("CURA_ORIGIN_SECRET")
        .map_err(|_| Error::RustError("CURA_ORIGIN_SECRET unset".into()))?
        .to_string();

    let canonical = format!(
        "POST\n/v1/healthcheck\n{}\n{}",
        timestamp,
        cura_crypto::sha256_hex(&[])
    );
    if !cura_crypto::verify(canonical.as_bytes(), &signature, secret.as_bytes()) {
        return Response::error("signature mismatch", 401);
    }

    match crate::healthcheck::run(&env).await {
        Ok(()) => Response::from_json(&serde_json::json!({"status": "ok"})),
        Err(e) => {
            let resp = Response::from_json(&serde_json::json!({"status": "error", "reason": e}))?;
            Ok(resp.with_status(500))
        }
    }
}

async fn post_weave(mut req: Request, env: Env, ctx: Context) -> Result<Response> {
    // Curator authenticates with CURA_ORIGIN_SECRET.
    let signature = match req.headers().get("x-cura-origin-signature")? {
        Some(s) => s,
        None => return Response::error("missing signature", 401),
    };

    let ts_header = req
        .headers()
        .get("x-cura-origin-timestamp")?
        .ok_or_else(|| Error::RustError("missing timestamp".into()))?;
    let timestamp: i64 = ts_header
        .parse()
        .map_err(|_| Error::RustError("invalid timestamp".into()))?;

    let now = (js_sys::Date::now()) as i64;
    if (now - timestamp).abs() > 300_000 {
        return Response::error("stale request", 401);
    }

    let body_bytes = req.bytes().await?;
    let secret = env
        .secret("CURA_ORIGIN_SECRET")
        .map_err(|_| Error::RustError("CURA_ORIGIN_SECRET unset".into()))?
        .to_string();

    let canonical = format!(
        "POST\n/v1/weaves\n{}\n{}",
        timestamp,
        cura_crypto::sha256_hex(&body_bytes)
    );
    if !cura_crypto::verify(canonical.as_bytes(), &signature, secret.as_bytes()) {
        return Response::error("signature mismatch", 401);
    }

    let body: PostBody =
        serde_json::from_slice(&body_bytes).map_err(|e| Error::RustError(format!("body: {e}")))?;

    // `plane` is a wire-compat label, not a selector: sink.write below
    // always writes intent, pattern and weave together (a weave with no
    // matching intent/pattern row is malformed, so there's no partial
    // write to opt into). This just sanity-checks the caller sent a
    // known value; curator's own Emitter fixes it to "weave" and has no
    // way to choose otherwise (see curator/src/emitter.rs).
    match body.plane.as_str() {
        "intent" | "pattern" | "weave" => {}
        other => return Response::error(format!("unknown plane: {other}"), 400),
    };

    // A statement with no RHS (a bare function signature, a closing
    // brace the sheath still emits a Read for) has nothing to key a
    // fact on. Reject it here, at the request boundary, as the
    // malformed input it is -- not three calls deeper inside D1Sink,
    // where the same condition surfaces as SinkError::Invalid and gets
    // reported as a 500. A missing intent is the caller's mistake, not
    // the server's.
    if body.output.is_empty() {
        return Response::error("empty output: nothing to record as an intent", 400);
    }

    // The intent is the RHS-visible output term; the pattern is
    // operands + intent joined by `_`; the weave is the operation plus
    // `,=` — same construction PlaneRow::from_sheathed_with_lang uses,
    // kept in step deliberately (see STATE.md).
    let intent = body.output.clone();
    let pattern = if body.input.is_empty() {
        intent.clone()
    } else {
        format!("{}_{}", body.input.replace(' ', "_"), intent)
    };
    let weave = format!("{},=", body.logic);
    let op = cura_protocol::Op::from_str(&body.op).unwrap_or(cura_protocol::Op::Change);

    let row = cura_registry::PlaneRow {
        intent: intent.clone(),
        binary: cura_registry::binary_of_str(&intent),
        binary_kind: cura_registry::kind_of_str(&intent).to_string(),
        binary_len: cura_registry::byte_len_of_str(&intent),
        pattern,
        weave,
        op,
        witness: body.source.clone(),
        source: body.source.clone(),
        source_lang: body.lang.clone(),
        observed_at_ms: now,
    };

    let sink = crate::d1_sink::D1Sink::from_env(&env)?;
    let key = format!("{}/{}", row.weave, row.pattern);
    let resp_intent = row.intent.clone();
    let resp_pattern = row.pattern.clone();
    let resp_weave = row.weave.clone();

    ctx.wait_until(async move {
        if let Err(e) = sink.write(&row).await {
            worker::console_error!("post_weave sink failed: {e}");
        }
    });

    Response::from_json(&serde_json::json!({
        "weave_id": key,
        "intent": resp_intent,
        "pattern": resp_pattern,
        "weave": resp_weave,
        "status": "stored",
        "plane": body.plane,
    }))
}
