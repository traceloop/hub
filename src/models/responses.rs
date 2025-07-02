use serde::Serialize;

#[derive(Serialize)]
pub struct ModelListResponse {
    pub object: String, // always "list"
    pub data: Vec<ModelInfoResponse>,
}

#[derive(Serialize)]
pub struct ModelInfoResponse {
    pub id: String,
    pub object: String, // always "model"
    pub owned_by: String,
}