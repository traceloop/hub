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
    pub slug: String, // the model_type e.g "gpt-3.5-turbo" clients send in the request "model" field
    pub provider: String, // provider type, e.g. "anthropic", as exposed in the provider header
}
