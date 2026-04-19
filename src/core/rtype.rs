//! Rigid-type (R-type) adaptors for multi-port connections.
//!
//! R-type adaptors use a scattering matrix to resolve the wave propagation
//! between multiple ports simultaneously. This is useful for modeling
//! complex junctions that cannot be easily decomposed into series and
//! parallel connections.
//!
//! ## Defining custom R-type adaptors
//!
//! Use the `define_rtype!` and `define_root_rtype!` proc macros from
//! `wdf_macros`. Pass the struct name and the **total port count**:
//!
//! ```ignore
//! use wdf_macros::{define_rtype, define_root_rtype};
//!
//! define_rtype!(My5Port, 5);        // 5 ports total: 1 parent + 4 children (p1..p4)
//! define_root_rtype!(MyRoot4Port, 4); // 4 children, no parent
//! ```

use crate::core::wdf::*;
use wdf_macros::{define_root_rtype, define_rtype};

define_rtype!(RType4Port, 4);
define_rtype!(RType5Port, 5);
define_rtype!(RType6Port, 6);
define_rtype!(RType7Port, 7);
define_rtype!(RType8Port, 8);

define_root_rtype!(RootRType4Port, 4);
define_root_rtype!(RootRType5Port, 5);
define_root_rtype!(RootRType6Port, 6);
