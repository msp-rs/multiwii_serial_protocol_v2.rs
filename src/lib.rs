//! Multiwii Serial Protocol (MSP) traffic decoder and structures
//!
//! Incomplete. Includes some structures from Cleanflight and Betaflight.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc))]

#[cfg(not(feature = "std"))]
#[macro_use]
extern crate alloc;

extern crate packed_struct;

extern crate crc_any;

#[macro_use]
extern crate packed_struct_codegen;

#[macro_use]
extern crate serde_derive;

extern crate serde;

mod prelude;

mod commands;
mod packet;
pub mod structs;

pub use commands::*;
pub use packet::*;
