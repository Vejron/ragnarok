//! Types and constants shared by the Ragnarok server and tools.
//!
//! The browser client's copies are generated from here: the messages and components
//! (`client/src/net/*.generated.ts`, by this crate's tests), the constants (`consts.generated.ts`, by
//! `npm run gen:consts`) and the tables (`tables.generated.json`). The flag and kind constants in
//! `client/src/net/codec.ts` are checked against these by `consts-parity.spec.ts`.

pub mod carve;
pub mod consts;
pub mod construct;
pub mod items;
pub mod mounts;
pub mod movement;
pub mod protocol;
pub mod quant;
pub mod replicated;
pub mod terrain;
pub mod vehicles;
pub mod world;
