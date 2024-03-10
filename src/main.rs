use actix_web::web;

mod config;
mod db;
mod errors;
mod server;


#[tokio::main]
async fn main() -> Result<(), errors::CustomError> {
    let cfg = config::load_config()?;
    println!("Config: {:?}", cfg);

    let pool = db::get_pool(cfg.db_conn_string.as_str(), cfg.db_n_max_connections).await?;
    let server_data = web::Data::new(server::MyData { pool });

    server::run_server(server_data, cfg.port).await
}
