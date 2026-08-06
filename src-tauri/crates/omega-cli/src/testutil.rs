//! Test-only utilities shared across the `omega` crate's unit tests.
//!
//! `std::env` is process-global, but Rust runs unit tests (`cargo test`) in
//! parallel threads. Any test in this crate that reads or writes `OMEGA_*`
//! environment variables must hold [`lock_env`] so concurrent tests can't
//! clobber each other's process-global state. This module is only compiled
//! under `#[cfg(test)]` and is not part of the production binary.

use std::sync::{Mutex, MutexGuard};

static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Acquire the crate-wide lock guarding `std::env` access in tests.
pub fn lock_env() -> MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap()
}
