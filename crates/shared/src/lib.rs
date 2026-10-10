//! Types and constants shared by the Ragnarok server and tools.
//!
//! The browser client mirrors everything in here by hand (`client/src/net/codec.ts`,
//! `client/src/sim/consts.ts`); byte-fixture tests on both sides pin the two together.

pub mod consts;
pub mod delta;
pub mod items;
pub mod movement;
pub mod protocol;
pub mod quant;
pub mod terrain;
pub mod world;
