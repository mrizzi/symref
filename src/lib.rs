pub mod deref;
pub mod naming;
pub mod store;
pub mod types;

// Re-export the main library functions and types for convenience
pub use deref::deref;
pub use store::store;
pub use types::{StoreOutput, VarRef, VarStore};
