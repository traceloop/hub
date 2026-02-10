use crate::models::streaming::ChatCompletionChunk;

/// Extract and concatenate text from accumulated streaming chunks.
/// Joins the delta content from all chunks into a single string.
pub fn extract_text_from_chunks(chunks: &[ChatCompletionChunk]) -> String {
    chunks
        .iter()
        .flat_map(|chunk| &chunk.choices)
        .filter_map(|choice| choice.delta.content.as_deref())
        .collect()
}
