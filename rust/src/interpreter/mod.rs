pub mod arithmetic;
pub(crate) mod arithmetic_meter;
mod binding_table;
pub(crate) mod bindings;
mod body_symbols;
pub(crate) mod broadcast_tree;
pub mod cast;
pub(crate) mod collection_meter;
pub mod comparison;
#[cfg(test)]
mod comparison_tests;
mod contract_record;
pub mod control;
pub(crate) mod declared_nil_contract;
pub(crate) mod dense_kernels;
#[cfg(test)]
mod dense_kernels_tests;
#[cfg(test)]
mod dense_quotient_tests;
#[cfg(test)]
mod dense_slice_route_tests;
pub(crate) mod engine;
#[cfg(test)]
mod error_message_format_tests;
#[cfg(test)]
mod error_operand_restore_tests;
pub(crate) mod exact_work;
pub mod execute_def;
#[cfg(test)]
mod format_json_tests;
mod format_ops;
pub(crate) mod fused_block;
#[cfg(test)]
mod fused_block_cache_tests;
mod fused_block_columns;
mod fused_block_general;
mod fused_block_lane;
#[cfg(test)]
mod fused_block_lane_call_tests;
mod fused_block_lower;
#[cfg(test)]
mod fused_block_over_zero_tests;
#[cfg(test)]
mod fused_block_tests;
#[cfg(test)]
mod fused_block_tier_tests;
pub mod higher_order;
pub mod higher_order_fold;
#[cfg(test)]
mod higher_order_tests;
pub mod host;
mod quickened;
#[cfg(test)]
mod quickened_tests;
#[cfg(test)]
mod route_observation;
mod truth_columns;
mod truth_ops;
// The host-side Word lookup lives in `host.rs`; the module path it used to
// have is kept for the hosts (`wasm_interpreter_bindings`, `rust/tests`) that
// import it by that name.
pub use self::host as host_lookup;
pub(crate) mod host_profile_defaults;
pub mod io;
mod json_decode;
mod json_encode;
pub(crate) mod lane_lift;
// The limit-profile enumeration lives in `host_profile_defaults.rs`; the
// module path it used to have is kept for the agent profile and the wasm
// bindings, which import it by that name.
pub(crate) use self::host_profile_defaults as limit_profile;
mod ceiling_refusal;
pub mod interpreter_core;
pub mod logic;
pub mod math_ops;
pub(crate) mod naming_convention_checker;
mod ordering_ops;
#[cfg(test)]
mod ordering_ops_tests;
#[cfg(test)]
mod power_words_tests;
mod record_ops;
#[cfg(test)]
mod record_words_tests;
mod reflection_ops;
#[cfg(test)]
mod reflection_words_tests;
pub mod runtime_limits;
mod search_ops;
mod session_lifecycle;
mod shape_words;
#[cfg(test)]
mod shape_words_tests;
pub mod sort;
pub mod tensor_cmds;
pub(crate) mod tensor_lane_ops;
pub mod tensor_ops;
mod value_digest;
pub(crate) mod value_extraction_helpers;
mod value_order;
pub mod vector_ops;
pub mod word_contract;
mod word_contract_flow;
#[cfg(test)]
mod word_contract_tests;
mod word_identity;
#[cfg(test)]
mod word_identity_tests;
#[cfg(test)]
mod work_meter_calibration_tests;

mod resolve_word;

mod execution_loop;
#[cfg(test)]
mod execution_step_parity_tests;
mod value_as_code;

mod execute_builtin;

mod failure_site;
pub(crate) mod fusion_contract;
#[cfg(test)]
mod fusion_contract_route_tests;
#[cfg(test)]
mod fusion_contract_tests;
pub(crate) mod nil_diagnostics;
#[cfg(test)]
mod route_differential_tests;

#[cfg(test)]
mod arithmetic_meter_tests;
#[cfg(test)]
mod collection_meter_tests;
#[cfg(test)]
mod definable_name_tests;
#[cfg(test)]
mod definition_source_tests;
#[cfg(test)]
mod dependents_index_tests;
#[cfg(test)]
mod dictionary_operation_tests;
#[cfg(test)]
mod dictionary_resolution_tests;
#[cfg(test)]
mod dictionary_tier_tests;
#[cfg(test)]
mod exact_vector_broadcast_tests;
#[cfg(test)]
mod higher_order_block_plan_tests;
#[cfg(test)]
mod higher_order_column_tests;
#[cfg(test)]
mod index_projection_tests;
#[cfg(test)]
mod interpreter_definition_tests;
#[cfg(test)]
mod interpreter_execution_tests;
#[cfg(test)]
mod interpreter_mode_tests;
#[cfg(test)]
mod kleene_truth_conformance_tests;
#[cfg(test)]
mod math_ops_tests;
#[cfg(test)]
mod nil_conformance_tests;
#[cfg(test)]
mod nil_contract_conformance_tests;
#[cfg(test)]
mod nil_diagnostics_tests;
#[cfg(test)]
mod nil_reason_tests;
#[cfg(test)]
mod reconcile_conformance_tests;
#[cfg(test)]
mod text_meter_tests;

pub use interpreter_core::*;
pub use runtime_limits::RuntimeLimits;

pub use host::{default_host_env, DefaultHostEnv, HostEffect, HostEnv, RecordingHostEnv};

pub use crate::types::WordDefinition;

#[cfg(test)]
mod builtin_dispatch_tests;
#[cfg(test)]
mod core_word_canonicalization_tests;
#[cfg(test)]
mod scalar_fastpath_tests;
#[cfg(test)]
mod word_domains_tests;
