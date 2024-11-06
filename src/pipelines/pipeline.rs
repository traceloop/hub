use crate::config::models::{Pipeline, PipelineType};
use axum::http::HeaderMap;

pub fn select_pipeline<'a>(
    pipelines: &'a [Pipeline],
    pipeline_type: PipelineType,
    headers: &HeaderMap,
) -> Option<&'a Pipeline> {
    // Filter pipelines by type
    let matching_pipelines: Vec<&Pipeline> = pipelines
        .iter()
        .filter(|p| p.r#type == pipeline_type)
        .collect();

    if matching_pipelines.is_empty() {
        return None;
    }

    if matching_pipelines.len() == 1 {
        return Some(matching_pipelines[0]);
    }

    // Check for pipeline specification in headers
    if let Some(pipeline_name) = headers
        .get("x-traceloop-pipeline")
        .and_then(|h| h.to_str().ok())
    {
        matching_pipelines
            .into_iter()
            .find(|p| p.name == pipeline_name)
    } else {
        // Default to pipeline named "default"
        matching_pipelines.into_iter().find(|p| p.name == "default")
    }
}
