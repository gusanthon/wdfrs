//! WDF components, adaptors, traits, and handles.

mod adaptors;
mod builder;
mod combos;
mod leaf;
mod nonlinear;
mod roots;
mod traits;
mod wdf_handle;

pub use adaptors::*;
pub use builder::*;
pub use combos::*;
pub use leaf::*;
pub use nonlinear::*;
pub use roots::*;
pub use traits::*;
pub use wdf_handle::*;

/// Convenience alias for a pinned, heap-allocated root node.
pub type WDFRoot<R> = core::pin::Pin<Box<R>>;
