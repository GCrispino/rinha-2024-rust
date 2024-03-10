use std::{io, fmt, num};
use actix_web::{http, HttpResponse};

#[derive(Debug)]
pub enum CustomError {
    ParseIntError(num::ParseIntError),
    IoError(std::io::Error),
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


#[derive(Debug)]
pub enum AppError {
    ErrNegativeTransactionBalance,
    ErrCustomerNotFound,
    SQLError(sqlx::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
