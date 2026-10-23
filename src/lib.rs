#![allow(async_fn_in_trait)]

pub mod agent;
pub mod browser;
#[cfg(not(target_arch = "wasm32"))]
pub mod i18n;
pub mod platform;
pub mod workspace;
