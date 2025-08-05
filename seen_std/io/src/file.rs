//! File I/O for Seen

use std::fs;
use seen_common::SeenResult;

/// Read a file to string
pub fn read_file(path: &str) -> SeenResult<String> {
    fs::read_to_string(path)
        .map_err(|e| seen_common::SeenError::io_error(e.to_string()))
}