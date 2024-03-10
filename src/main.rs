use std::{env, fmt, io, num};

use actix_web::error::{ErrorInternalServerError, ErrorUnprocessableEntity};
use actix_web::{http, middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use env_logger;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::chrono::{Local, NaiveDateTime};

#[derive(Debug)]
enum CustomError {
    ParseIntError(num::ParseIntError),
    IoError(io::Error),
    SQLError(sqlx::Error),
    StringError(String),
    StandardError(Box<dyn std::error::Error>),
}

impl From<num::ParseIntError> for CustomError {
    fn from(error: num::ParseIntError) -> Self {
        CustomError::ParseIntError(error)
    }
}

impl From<io::Error> for CustomError {
    fn from(error: io::Error) -> Self {
        CustomError::IoError(error)
    }
}

impl From<sqlx::Error> for CustomError {
    fn from(error: sqlx::Error) -> Self {
        CustomError::SQLError(error)
    }
}

struct MyData {
    pool: sqlx::Pool<sqlx::Postgres>,
}

async fn statement(
    id: web::Path<i64>,
    d: web::Data<MyData>,
    _: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let statement_result = get_statement_db(d.pool.to_owned(), id.clone()).await?;

    let customer = statement_result.0;
    let transactions = statement_result.1;

    let txs = transactions
        .iter()
        .map(StatementTransaction::from)
        .collect();

    let statement = GetCustomerStatementResponse {
        balance: Balance {
            total: customer.balance,
            limit: customer.limit,
            date: Local::now().naive_utc(),
        },
        last_transactions: txs,
    };

    let res = serde_json::to_string(&statement).map_err(ErrorUnprocessableEntity)?;
    Ok(HttpResponse::Ok().body(res))
}

async fn create_transaction(
    id: web::Path<i32>,
    create_transaction_data: web::Json<CreateCustomerTransactionRequest>,
    d: web::Data<MyData>,
    _: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let request = create_transaction_data.into_inner();

    let tx_type = request.tx_type;

    match tx_type.as_str() {
        "d" | "c" => {}
        _ => {
            return Err(ErrorUnprocessableEntity("tipo de transação invalido"));
        }
    }

    let desc_length = request.description.len();

    if desc_length == 0 || desc_length > 10 {
        return Err(ErrorUnprocessableEntity("tamanho de descrição inválido"));
    }

    let (limit, total) = create_customer_transaction_db(
        d.pool.to_owned(),
        id.clone(),
        request.value,
        tx_type,
        request.description,
    )
    .await?;

    let res = serde_json::to_string(&CreateCustomerTransactionResponse { limit, total })
        .map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().body(res))
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

struct Transaction {
    id: Option<i32>,
    value: Option<i32>,
    tx_type: Option<String>,
    description: Option<String>,
    customer_id: Option<i32>,
    created_at: Option<NaiveDateTime>,
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

struct Customer {
    id: i32,
    limit: i32,
    balance: i32,
    created_at: NaiveDateTime,
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

#[derive(Debug, Serialize, Deserialize)]
struct GetCustomerStatementResponse {
    #[serde(rename = "saldo")]
    balance: Balance,
    #[serde(rename = "ultimas_transacoes")]
    last_transactions: Vec<StatementTransaction>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateCustomerTransactionRequest {
    #[serde(rename = "valor")]
    value: i32,
    #[serde(rename = "tipo")]
    tx_type: String,
    #[serde(rename = "descricao")]
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CreateCustomerTransactionResponse {
    #[serde(rename = "limite")]
    limit: i64,
    #[serde(rename = "saldo")]
    total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Balance {
    total: i32,
    #[serde(rename = "limite")]
    limit: i32,
    #[serde(rename = "data_extrato")]
    date: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatementTransaction {
    #[serde(rename = "valor")]
    value: Option<i32>,
    #[serde(rename = "tipo")]
    tx_type: Option<String>,
    #[serde(rename = "descricao")]
    description: Option<String>,
    #[serde(rename = "realizada_em")]
    date: Option<NaiveDateTime>,
}

impl From<&Transaction> for StatementTransaction {
    fn from(db_tx: &Transaction) -> Self {
        StatementTransaction {
            value: db_tx.value,
            tx_type: db_tx.tx_type.clone(),
            description: db_tx.description.clone(),
            date: db_tx.created_at,
        }
    }
}

#[derive(Debug)]
enum AppError {
    ErrNegativeTransactionBalance,
    ErrCustomerNotFound,
    SQLError(sqlx::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        println!("FMT! !");
        match *self {
            AppError::ErrNegativeTransactionBalance => {
                write!(f, "operation results in negative transaction balance")
            }
            AppError::ErrCustomerNotFound => write!(f, "customer not found"),
            // The wrapped error contains additional information and is available
            // via the source() method.
            AppError::SQLError(..) => write!(f, "sql error"),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> AppError {
        AppError::SQLError(err)
    }
}

impl actix_web::error::ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(http::header::ContentType::plaintext())
            .body(self.to_string())
    }
    fn status_code(&self) -> http::StatusCode {
        match *self {
            AppError::ErrNegativeTransactionBalance => http::StatusCode::UNPROCESSABLE_ENTITY,
            AppError::ErrCustomerNotFound => http::StatusCode::NOT_FOUND,
            AppError::SQLError(..) => http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

async fn get_statement_db(
    pool: sqlx::Pool<sqlx::Postgres>,
    id: i64,
) -> Result<(Customer, Vec<Transaction>), AppError> {
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
        return Err(AppError::ErrCustomerNotFound);
    }

    let first_res = statement_query_res
        .first()
        .ok_or(AppError::ErrCustomerNotFound)?;
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

async fn create_customer_transaction_db(
    pool: sqlx::Pool<sqlx::Postgres>,
    customer_id: i32,
    value: i32,
    tx_type: String,
    description: String,
) -> Result<(i64, i64), AppError> {
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
            sqlx::Error::RowNotFound => AppError::ErrCustomerNotFound,
            _ => err.into(),
        })?;

    if update_count == 0 {
        return Err(AppError::ErrNegativeTransactionBalance);
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

async fn get_pool(
    conn_string: &str,
    n_max_connections: u32,
) -> Result<sqlx::Pool<sqlx::Postgres>, CustomError> {
    // Create a connection pool
    let pool = PgPoolOptions::new()
        .max_connections(n_max_connections)
        .connect(conn_string)
        .await?;

    Ok(pool)
}

async fn run_server(data: web::Data<MyData>, port: u16) -> Result<(), CustomError> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("debug"));

    let _ = HttpServer::new(
        move || {
            App::new()
                .service(web::resource("/clientes/{id}/extrato").route(web::get().to(statement)))
                .service(
                    web::resource("/clientes/{id}/transacoes")
                        .route(web::post().to(create_transaction)),
                )
                // enable logger
                .wrap(middleware::Logger::default())
                .app_data(data.clone())
        }, // add shared state
    )
    .bind(("0.0.0.0", port))?
    .run()
    .await?;
    Ok(())
}

const PORT: u16 = 8080;
const DEFAULT_DB_N_MAX_CONNECTIONS: u32 = 5;
const DEFAULT_DB_CONN_STRING: &str = "postgres://user:password@localhost/rinha";

#[derive(Debug)]
struct Config {
    port: u16,
    db_n_max_connections: u32,
    db_conn_string: String,
}

fn load_config() -> Result<Config, CustomError> {
    let args: Vec<String> = env::args().collect();
    let mut port = PORT;
    if args.len() > 2 {
        return Err(CustomError::StringError(
            "args length should be max 1".to_string(),
        ));
    }
    if args.len() != 1 {
        port = args[1].parse::<u16>()?;
    }

    let db_n_max_connections: u32 = env::var("DB_MAX_OPEN_CONNS")
        .map_err(|err| CustomError::StandardError(Box::new(err)))
        .and_then(|n_str| n_str.parse::<u32>().map_err(CustomError::ParseIntError))
        .unwrap_or(DEFAULT_DB_N_MAX_CONNECTIONS);

    let db_conn_string = env::var("DB_CONN_STR").unwrap_or(DEFAULT_DB_CONN_STRING.to_string());

    Ok(Config {
        port,
        db_n_max_connections,
        db_conn_string,
    })
}

#[tokio::main]
async fn main() -> Result<(), CustomError> {
    let cfg = load_config()?;
    println!("Config: {:?}", cfg);

    let pool = get_pool(cfg.db_conn_string.as_str(), cfg.db_n_max_connections).await?;
    let server_data = web::Data::new(MyData { pool });

    run_server(server_data, cfg.port).await
}
