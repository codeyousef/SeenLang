//! Test module for the Seen type checker

mod primitive_types_test;
mod type_inference_test;
mod function_types_test;
mod struct_types_test;
mod array_types_test;
mod control_flow_test;
mod operator_types_test;
mod type_compatibility_test;
mod scope_test;
mod error_recovery_test;
mod test_helpers;

// Re-export test utilities
pub use test_helpers::*;
