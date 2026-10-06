use crate::db::models::{NewTransaction, Transaction};
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct TransactionRepo {
    pool: PgPool,
}

impl TransactionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, tx: &NewTransaction) -> Result<String> {
        let id = Uuid::new_v4().to_string();
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
           tx.value_usd as f64,
           tx.token,
           tx.token_symbol,
           tx.block_number,
           tx.tx_timestamp,
        )
        .execute(&self.pool)
        .await?;
        Ok(id)
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