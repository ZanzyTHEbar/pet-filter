//! Integration test driver for `tests/integration/` submodule.
//!
//! Each `mod` below maps to a file that exercises a specific subsystem
//! against mock adapters.  All tests run on the host (x86_64) with no
//! real hardware required.

mod app_service_tests;
mod auth_tests;
mod mock_hw;
mod provisioning_flow_tests;
