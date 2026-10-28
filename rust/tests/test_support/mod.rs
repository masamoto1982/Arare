//! Shared test-support modules for the property-based law suites.
//!
//! `generators` provides semantic-domain Ajisai-source strategies and
//! `observe` provides the firewall-clean LANG.OBSERVATION.FIREWALL axis observation and the pure
//! `render : (data, role) → display` function (Phase 1 foundation).

pub mod generators;
pub mod observe;
