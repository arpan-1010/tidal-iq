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
}