use std::env;

pub fn stream_buffer_size_bytes() -> usize {
    env::var("STREAM_BUFFER_SIZE_BYTES")
        .unwrap_or_else(|_| "1000".to_string())
        .parse::<usize>()
        .unwrap_or(1000)
}
