//! Management API handlers (Axum routes).

pub mod apps;
pub mod backend;
pub mod profiles;
pub mod status;

pub use apps::*;
pub use backend::*;
pub use profiles::*;
pub use status::*;
