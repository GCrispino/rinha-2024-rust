use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::chrono::NaiveDateTime;

use crate::errors;

pub struct Customer {
    pub id: i32,
    pub limit: i32,
    pub balance: i32,
    pub created_at: NaiveDateTime,
}

pub struct Transaction {
    pub id: Option<i32>,
    pub value: Option<i32>,
    pub tx_type: Option<String>,
    pub description: Option<String>,
    pub customer_id: Option<i32>,
    pub created_at: Option<NaiveDateTime>,
}

#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
struct GetCustomerStatementResult {
    // customer data
    customer_id: i32,
    customer_limit: i32,
    customer_balance: i32,
    customer_created_at: NaiveDateTime,
    // transaction data
    transaction_id: Option<i32>,
    transaction_value: Option<i32>,
    transaction_type: Option<String>,
    transaction_description: Option<String>,
    transaction_customer_id: Option<i32>,
    transaction_created_at: Option<NaiveDateTime>,
}

impl From<GetCustomerStatementResult> for Transaction {
    fn from(customer_statement: GetCustomerStatementResult) -> Self {
        Transaction {
            id: customer_statement.transaction_id,
            value: customer_statement.transaction_value,
            tx_type: customer_statement.transaction_type,
            description: customer_statement.transaction_description,
            customer_id: customer_statement.transaction_customer_id,
            created_at: customer_statement.transaction_created_at,
        }
    }
}

impl From<&GetCustomerStatementResult> for Customer {
    fn from(customer_statement: &GetCustomerStatementResult) -> Self {
        Customer {
            id: customer_statement.customer_id,
            limit: customer_statement.customer_limit,
            balance: customer_statement.customer_balance,
            created_at: customer_statement.customer_created_at,
        }
    }
}

pub async fn get_statement_db(
    pool: sqlx::Pool<sqlx::Postgres>,
    id: i64,
) -> Result<(Customer, Vec<Transaction>), errors::AppError> {
    let query = "
		SELECT 
            c.id as customer_id,
            c.limit as customer_limit,
            c.balance as customer_balance,
            c.created_at as customer_created_at,
            t.id as transaction_id,
            t.value as transaction_value,
            t.type as transaction_type,
            t.description as transaction_description,
            t.customer_id as transaction_customer_id,
            t.created_at as transaction_created_at
        FROM customers c
		LEFT JOIN transactions t ON c.id=t.customer_id
		WHERE c.id = $1
		ORDER BY t.created_at DESC
		LIMIT 10
	";

    let statement_query_res = sqlx::query_as::<_, GetCustomerStatementResult>(query)
        .bind(id)
        .fetch_all(&pool)
        .await?;

    if statement_query_res.len() == 0 {
        return Err(errors::AppError::ErrCustomerNotFound);
    }

    let first_res = statement_query_res
        .first()
        .ok_or(errors::AppError::ErrCustomerNotFound)?;
    let customer: Customer = Customer::from(first_res);
    let mut txs: Vec<Transaction> = vec![];
    if statement_query_res.len() >= 1 {
        let fst = statement_query_res.first().unwrap();
        if fst.transaction_id.is_some() {
            txs = statement_query_res
                .into_iter()
                .map(Transaction::from)
                .collect();
        }
    }

    Ok((customer, txs))
}

pub async fn create_customer_transaction_db(
    pool: sqlx::Pool<sqlx::Postgres>,
    customer_id: i32,
    value: i32,
    tx_type: String,
    description: String,
) -> Result<(i64, i64), errors::AppError> {
    // TODO -> add rollbacks if needed
    let mut tx = pool.begin().await?;

    let update_query = "
		with
			c AS (SELECT * FROM customers c WHERE id = $2),
			u AS (
				UPDATE customers c2 SET balance = balance + $1
				WHERE id = $2 AND (balance + $1) >= -\"limit\"
				RETURNING id, \"limit\", balance
			),
			cu AS (SELECT COUNT(*) FROM u)
		SELECT c.limit, c.balance, cu.count as count_update FROM c, cu
    ";

    let insert_query = "
      INSERT INTO transactions (value, \"type\", description, customer_id)
      VALUES ($1, $2, $3, $4)
    ";

    let mut update_value = value as i64;
    if tx_type == "d" {
        update_value = -update_value
    }

    let (limit, total, update_count): (i32, i32, i64) = sqlx::query_as(update_query)
        .bind(update_value)
        .bind(customer_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|err| match err {
            sqlx::Error::RowNotFound => errors::AppError::ErrCustomerNotFound,
            _ => err.into(),
        })?;

    if update_count == 0 {
        return Err(errors::AppError::ErrNegativeTransactionBalance);
    }

    let _ = sqlx::query(insert_query)
        .bind(value)
        .bind(tx_type)
        .bind(description)
        .bind(customer_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok((limit as i64, (total as i64) + update_value))
}

pub async fn get_pool(
    conn_string: &str,
    n_max_connections: u32,
) -> Result<sqlx::Pool<sqlx::Postgres>, errors::CustomError> {
    // Create a connection pool
    let pool = PgPoolOptions::new()
        .max_connections(n_max_connections)
        .connect(conn_string)
        .await?;

    Ok(pool)
}
