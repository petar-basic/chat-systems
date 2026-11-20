pub mod google;
pub mod messages;
pub mod models;
pub mod outlook;
pub mod repo;
pub mod routes;
pub mod service;
pub mod worker;

use shared_common::errors::AppError;

#[derive(Debug)]
pub enum SyncError {
    Gone,
    Failed(AppError),
}
