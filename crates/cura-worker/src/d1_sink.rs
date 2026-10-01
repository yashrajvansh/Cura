//! D1-backed sink. Three databases, one per plane.
//!
//! `write` returns a future the caller drives:
//!   - fetch handler: ctx.wait_until(fut) — the runtime completes it
//!     after the response is sent.
//!   - scheduled handler: fut.await — the handler waits for it.
//!
//! Never call spawn_local from inside the sink. A future with no
//! owner is killed when the request or event ends — this is what
//! silently dropped every write in the first live deploy (2026-09-15),
//! and the reason the fix after that made the caller supply a
//! `Context` for `ctx.wait_until` at all. This version goes one step
//! further: the sink itself no longer calls `wait_until` internally
//! either, because that hid the future from any caller (a scheduled
//! handler included) who might need to await it directly instead of
//! firing it into the background — which the healthcheck below does.
use cura_registry::{PlaneRow, SinkError};
use worker::*;

pub struct D1Sink {
    db_di: D1Database,
    db_dp: D1Database,
    db_dip: D1Database,
}

impl D1Sink {
    pub fn from_env(env: &Env) -> Result<Self> {
        Ok(D1Sink {
            db_di: env.d1("DB_DI")?,
            db_dp: env.d1("DB_DP")?,
            db_dip: env.d1("DB_DIP")?,
        })
    }

    pub async fn write(&self, row: &PlaneRow) -> std::result::Result<(), SinkError> {
        self.put_intent(row).await?;
        self.put_pattern(row).await?;
        self.put_weave(row).await?;
        Ok(())
    }

    async fn put_intent(&self, row: &PlaneRow) -> std::result::Result<(), SinkError> {
        if row.intent.is_empty() {
            return Err(SinkError::Invalid("empty intent".into()));
        }
        let now_f = row.observed_at_ms as f64;
        self.db_di
            .prepare(
                "INSERT INTO intent \
                 (value, binary, binary_kind, binary_len, first_seen, last_seen) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5) \
                 ON CONFLICT (value) DO UPDATE SET \
                   last_seen = ?5, \
                   binary    = ?2, \
                   binary_kind = ?3, \
                   binary_len  = ?4",
            )
            .bind(&[
                row.intent.clone().into(),
                row.binary.clone().into(),
                row.binary_kind.clone().into(),
                (row.binary_len as f64).into(),
                now_f.into(),
            ])
            .map_err(|e| SinkError::Io(format!("bind intent: {e}")))?
            .run()
            .await
            .map_err(|e| SinkError::Io(format!("run intent: {e}")))?;
        Ok(())
    }

    async fn put_pattern(&self, row: &PlaneRow) -> std::result::Result<(), SinkError> {
        if row.pattern.is_empty() {
            return Err(SinkError::Invalid("empty pattern".into()));
        }
        let now_f = row.observed_at_ms as f64;
        self.db_dp
            .prepare(
                "INSERT INTO pattern (pattern, intent, first_seen, last_seen) \
                 VALUES (?1, ?2, ?3, ?3) \
                 ON CONFLICT (pattern) DO UPDATE SET last_seen = ?3",
            )
            .bind(&[
                row.pattern.clone().into(),
                row.intent.clone().into(),
                now_f.into(),
            ])
            .map_err(|e| SinkError::Io(format!("bind pattern: {e}")))?
            .run()
            .await
            .map_err(|e| SinkError::Io(format!("run pattern: {e}")))?;
        Ok(())
    }

    async fn put_weave(&self, row: &PlaneRow) -> std::result::Result<(), SinkError> {
        if row.weave.is_empty() {
            return Err(SinkError::Invalid("empty weave".into()));
        }
        let now_f = row.observed_at_ms as f64;

        // Language write first, then weave — so a query right after
        // never sees a fact with no language. Kept from the prior
        // version rather than following the plan's write-then-lang
        // order, which reintroduces exactly that window.
        self.db_dip
            .prepare(
                "INSERT INTO weave_lang \
                 (weave, pattern, intent, source_lang, first_seen, last_seen) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5) \
                 ON CONFLICT (weave, pattern, intent, source_lang) DO UPDATE SET \
                   last_seen = MAX(last_seen, ?5)",
            )
            .bind(&[
                row.weave.clone().into(),
                row.pattern.clone().into(),
                row.intent.clone().into(),
                row.source_lang.clone().into(),
                now_f.into(),
            ])
            .map_err(|e| SinkError::Io(format!("bind weave_lang: {e}")))?
            .run()
            .await
            .map_err(|e| SinkError::Io(format!("run weave_lang: {e}")))?;

        self.db_dip
            .prepare(
                "INSERT INTO weave \
                 (weave, pattern, intent, op, witness, source_lang, \
                  observed_at, occurrences, first_seen, last_seen) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?7, ?7) \
                 ON CONFLICT (weave, pattern, intent) DO UPDATE SET \
                   occurrences = occurrences + 1, \
                   last_seen   = MAX(last_seen, ?7)",
            )
            .bind(&[
                row.weave.clone().into(),
                row.pattern.clone().into(),
                row.intent.clone().into(),
                row.op_str().into(),
                row.witness.clone().into(),
                row.source_lang.clone().into(),
                now_f.into(),
            ])
            .map_err(|e| SinkError::Io(format!("bind weave: {e}")))?
            .run()
            .await
            .map_err(|e| SinkError::Io(format!("run weave: {e}")))?;

        Ok(())
    }
}
