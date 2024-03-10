use std::{env};

use crate::errors;

const PORT: u16 = 8080;
const DEFAULT_DB_N_MAX_CONNECTIONS: u32 = 5;
const DEFAULT_DB_CONN_STRING: &str = "postgres://user:password@localhost/rinha";


#[derive(Debug)]
pub struct Config {
    pub port: u16,
    pub db_n_max_connections: u32,
    pub db_conn_string: String,
}

pub fn load_config() -> Result<Config, errors::CustomError> {
    let args: Vec<String> = env::args().collect();
    let mut port = PORT;
    if args.len() > 2 {
        return Err(errors::CustomError::StringError(
            "args length should be max 1".to_string(),
        ));
    }
    if args.len() != 1 {
        port = args[1].parse::<u16>()?;
    }

    let db_n_max_connections: u32 = env::var("DB_MAX_OPEN_CONNS")
        .map_err(|err| errors::CustomError::StandardError(Box::new(err)))
        .and_then(|n_str| n_str.parse::<u32>().map_err(errors::CustomError::ParseIntError))
        .unwrap_or(DEFAULT_DB_N_MAX_CONNECTIONS);

    let db_conn_string = env::var("DB_CONN_STR").unwrap_or(DEFAULT_DB_CONN_STRING.to_string());

    Ok(Config {
        port,
        db_n_max_connections,
        db_conn_string,
    })
}
