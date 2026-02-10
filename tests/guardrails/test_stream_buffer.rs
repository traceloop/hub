use hub_lib::guardrails::stream_buffer::*;

use super::helpers::*;

// ---------------------------------------------------------------------------
// Phase 2: Stream Buffer (1 test)
// ---------------------------------------------------------------------------

#[test]
fn test_extract_from_accumulated_stream_chunks() {
    let chunks = vec![
        create_test_chunk("Hello"),
        create_test_chunk(" "),
        create_test_chunk("world"),
        create_test_chunk("!"),
    ];
    let text = extract_text_from_chunks(&chunks);
    assert_eq!(text, "Hello world!");
}
