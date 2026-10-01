//! Scheduled round-trip healthcheck. Runs on every cron fire, writes
//! a marker fact through the real D1Sink (the same path every real
//! curator translate run uses), reads it back through a direct D1
//! query, deletes it from all three planes. Logs "cura healthcheck
//! ok" on success, or the reason on failure.
//!
//! Tests the sink path and D1 connectivity end to end. Does not test
//! the HTTP handler or the HMAC verification in front of it — those
//! are exercised by every real curator translate run instead.

use cura_registry::PlaneRow;
use worker::*;

pub async fn run(env: &Env) -> std::result::Result<(), String> {
    let now = js_sys::Date::now() as i64;
    let marker = format!("__hc_{now}__");
    let pattern = format!("{marker}_hc");

    let row = PlaneRow {
        intent: marker.clone(),
        binary: cura_registry::binary_of_str(&marker),
        binary_kind: cura_registry::kind_of_str(&marker).to_string(),
        binary_len: cura_registry::byte_len_of_str(&marker),
        pattern: pattern.clone(),
        weave: "healthcheck,=".to_string(),
        op: cura_protocol::Op::Change,
        witness: "healthcheck".to_string(),
        source: "internal".to_string(),
        source_lang: "internal".to_string(),
        observed_at_ms: now,
    };

    let sink = crate::d1_sink::D1Sink::from_env(env).map_err(|e| format!("sink construct: {e}"))?;
    sink.write(&row)
        .await
        .map_err(|e| format!("sink write: {e}"))?;

    let read_result = read_back(env, &marker).await;

    // Cleanup runs regardless of whether the read-back matched -- a
    // leftover marker row is the failure mode this healthcheck exists
    // to catch, not one it should also cause.
    cleanup(env, &marker, &pattern).await;

    match read_result? {
        Some(source_lang) if source_lang == "internal" => {
            console_log!("cura healthcheck ok");
            Ok(())
        }
        Some(other) => Err(format!(
            "healthcheck: marker {marker} found but source_lang was {other:?}, not \"internal\""
        )),
        None => Err(format!(
            "healthcheck: marker {marker} not found after write"
        )),
    }
}

#[derive(serde::Deserialize)]
struct WeaveRow {
    source_lang: String,
}

async fn read_back(env: &Env, marker: &str) -> std::result::Result<Option<String>, String> {
    let db = env.d1("DB_DIP").map_err(|e| format!("dip binding: {e}"))?;
    let found: Option<WeaveRow> = db
        .prepare("SELECT source_lang FROM weave WHERE intent = ?1")
        .bind(&[marker.into()])
        .map_err(|e| format!("bind select: {e}"))?
        .first(None)
        .await
        .map_err(|e| format!("select: {e}"))?;
    Ok(found.map(|r| r.source_lang))
}

async fn cleanup(env: &Env, marker: &str, pattern: &str) {
    if let Ok(dip) = env.d1("DB_DIP") {
        if let Ok(stmt) = dip
            .prepare("DELETE FROM weave WHERE intent = ?1")
            .bind(&[marker.into()])
        {
            let _ = stmt.run().await;
        }
        if let Ok(stmt) = dip
            .prepare("DELETE FROM weave_lang WHERE intent = ?1")
            .bind(&[marker.into()])
        {
            let _ = stmt.run().await;
        }
    }
    if let Ok(di) = env.d1("DB_DI") {
        if let Ok(stmt) = di
            .prepare("DELETE FROM intent WHERE value = ?1")
            .bind(&[marker.into()])
        {
            let _ = stmt.run().await;
        }
    }
    if let Ok(dp) = env.d1("DB_DP") {
        if let Ok(stmt) = dp
            .prepare("DELETE FROM pattern WHERE pattern = ?1")
            .bind(&[pattern.into()])
        {
            let _ = stmt.run().await;
        }
    }
}
