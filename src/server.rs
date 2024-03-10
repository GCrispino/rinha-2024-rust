use actix_web::error::{ErrorInternalServerError, ErrorUnprocessableEntity};
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer};
use env_logger;
use serde::{Deserialize, Serialize};
use sqlx::types::chrono::{Local, NaiveDateTime};

use crate::{db, errors};

pub struct MyData {
    pub pool: sqlx::Pool<sqlx::Postgres>,
}

pub async fn statement(
    id: web::Path<i64>,
    d: web::Data<MyData>,
    _: HttpRequest,
) -> Result<HttpResponse, actix_web::Error> {
    let statement_result = db::get_statement_db(d.pool.to_owned(), id.clone()).await?;

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

    let (limit, total) = db::create_customer_transaction_db(
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

impl From<&db::Transaction> for StatementTransaction {
    fn from(db_tx: &db::Transaction) -> Self {
        StatementTransaction {
            value: db_tx.value,
            tx_type: db_tx.tx_type.clone(),
            description: db_tx.description.clone(),
            date: db_tx.created_at,
        }
    }
}

pub async fn run_server(data: web::Data<MyData>, port: u16) -> Result<(), errors::CustomError> {
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
