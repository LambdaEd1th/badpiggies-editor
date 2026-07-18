#![forbid(unsafe_code)]

//! UI-free editor logic shared by the Dioxus app and Web Worker.

pub mod data;
pub mod diagnostics;
pub mod domain;
pub mod io;
pub mod unity_runtime;
pub mod worker_protocol;

mod parallel;

pub use diagnostics::error::{AppError as CoreError, AppResult as CoreResult};
