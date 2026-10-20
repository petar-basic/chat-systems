use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct DataList<T: ToSchema> {
    pub data: Vec<T>,
}

impl<T: ToSchema> From<Vec<T>> for DataList<T> {
    fn from(data: Vec<T>) -> Self {
        Self { data }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct StatusResponse {
    pub status: &'static str,
}

impl StatusResponse {
    pub const fn new(status: &'static str) -> Self {
        Self { status }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DataItem<T: ToSchema> {
    pub data: T,
}
