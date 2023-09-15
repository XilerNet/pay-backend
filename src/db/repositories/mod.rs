pub mod models;
mod sqlx_postgresql;

pub use sqlx_postgresql::SqlxPostgresqlRepository as Repository;
