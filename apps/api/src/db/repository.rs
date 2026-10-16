use crate::db::models::{NewTransaction, Transaction};
use anyhow::Result;
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use chrono::NaiveDateTime;
use serde::Serialize;

pub struct TransactionRepo {
    pool: PgPool,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct VolumePoint {
    pub bucket: NaiveDateTime,
    pub volume_usd: f64,
    pub tx_count: i64,
}

#[derive(Debug, Serialize)]
pub struct ChainStat {
    pub chain: String,
    pub count: i64,
    #[serde(with = "rust_decimal::serde::float_option")]
    pub volume_usd: Option<Decimal>,
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct InsightRow {
    pub hash: String,
    pub chain: String,
    pub token_symbol: String,
    pub value_usd: f64,
    pub impact_score: Option<f32>,
    pub is_anomaly: Option<bool>,
    pub anomaly_reason: Option<String>,
    pub ai_summary: Option<String>,
    pub tx_timestamp: chrono::NaiveDateTime,
}

impl TransactionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, tx: &NewTransaction) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let value_usd = Decimal::from_f64(tx.value_usd)
            .ok_or_else(|| anyhow::anyhow!("value_usd {} is not a valid decimal", tx.value_usd))?;
        sqlx::query!(
           r#"
           INSERT INTO transactions
            (id, hash, chain, from_address, to_address, value, value_usd, token, token_symbol, block_number, tx_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
           "#,
           id,
           tx.hash,
           tx.chain,
           tx.from_address,
           tx.to_address,
           tx.value,
           value_usd,
           tx.token,
           tx.token_symbol,
           tx.block_number,
           tx.tx_timestamp,
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn get_chain_stats(&self) -> Result<Vec<ChainStat>> {
        let rows = sqlx::query_as!(
            ChainStat,
            r#"
            SELECT chain AS "chain!", COUNT(*) AS "count!", SUM(value_usd) AS volume_usd
            FROM transactions
            GROUP BY chain
            "#
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_volume_series(&self, bucket: &str, hours: i32) -> Result<Vec<VolumePoint>, sqlx::Error> {
        sqlx::query_as::<_, VolumePoint>(
            r#"
            SELECT
                date_trunc($1, tx_timestamp) AS bucket,
                COALESCE(SUM(value_usd), 0)::float8 AS volume_usd,
                COUNT(*) AS tx_count
            FROM transactions
            WHERE tx_timestamp >= (now() AT TIME ZONE 'utc') - make_interval(hours => $2::int)
            GROUP BY 1
            ORDER BY 1
            "#,
        )
        .bind(bucket)
        .bind(hours)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_by_hash(&self, hash: &str) -> Result<Option<Transaction>> {
        let tx = sqlx::query_as!(
            Transaction,
            "SELECT * FROM transactions WHERE hash = $1 LIMIT 1",
            hash
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(tx)
    }

    pub async fn get_recent(&self, limit: i64, chain: Option<&str>) -> Result<Vec<Transaction>> {
        let txs = match chain {
            Some(c) => sqlx::query_as!(
                Transaction,
                "SELECT * FROM transactions WHERE chain = $1 ORDER BY tx_timestamp DESC LIMIT $2",
                c, limit
            )
            .fetch_all(&self.pool)
            .await?,

            None => sqlx::query_as!(
                Transaction,
                "SELECT * FROM transactions ORDER BY tx_timestamp DESC LIMIT $1",
                limit
            )
            .fetch_all(&self.pool)
            .await?,
        };
        Ok(txs)
    }

    pub async fn score_unscored(&self) -> Result<u64, sqlx::Error> {
        let res = sqlx::query(
            r#"
            UPDATE transactions
            SET impact_score = LEAST(100.0, GREATEST(0.0,
                (ln(value_usd::float8) - ln(100000.0))
                / (ln(100000000.0) - ln(100000.0)) * 100.0))::real
            WHERE impact_score IS NULL AND value_usd IS NOT NULL AND value_usd > 0
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    pub async fn flag_anomalies(&self) -> Result<u64, sqlx::Error> {
        let res = sqlx::query(
            r#"
            UPDATE transactions t
            SET is_anomaly = (t.value_usd::float8 >= 5.0 * s.avg),
                anomaly_reason = CASE
                    WHEN t.value_usd::float8 >= 5.0 * s.avg THEN
                        format('%sx the 7-day average for %s',
                            round((t.value_usd::float8 / s.avg)::numeric, 1), t.chain)
                    ELSE NULL
                END
            FROM (
                SELECT chain, AVG(value_usd::float8) AS avg
                FROM transactions
                WHERE tx_timestamp > (now() at time zone 'utc') - interval '7 days'
                GROUP BY chain
                HAVING COUNT(*) >= 20
            ) s
            WHERE t.chain = s.chain
            AND t.is_anomaly IS NULL
            AND t.impact_score IS NOT NULL
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    pub async fn get_unsummarized(
        &self,
        limit: i64,
    ) -> Result<Vec<(String, String, String, String, String, f64, Option<bool>, Option<String>)>, sqlx::Error> {
        sqlx::query_as(
            r#"
            SELECT hash, chain, from_address, to_address, token_symbol,
                value_usd::float8, is_anomaly, anomaly_reason
            FROM transactions
            WHERE ai_summary IS NULL AND impact_score >= 40
            ORDER BY tx_timestamp DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn set_summary(&self, hash: &str, summary: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE transactions SET ai_summary = $1 WHERE hash = $2")
            .bind(summary)
            .bind(hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_insights(
        &self,
        anomalies_only: bool,
        limit: i64,
    ) -> Result<Vec<InsightRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightRow>(
            r#"
            SELECT hash, chain, token_symbol, value_usd::float8 AS value_usd,
                impact_score, is_anomaly, anomaly_reason, ai_summary, tx_timestamp
            FROM transactions
            WHERE tx_timestamp > (now() at time zone 'utc') - interval '24 hours'
            AND impact_score IS NOT NULL
            AND ($1 = false OR is_anomaly = true)
            ORDER BY impact_score DESC, tx_timestamp DESC
            LIMIT $2
            "#,
        )
        .bind(anomalies_only)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
}