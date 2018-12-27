/// This crate provides a reader that decodes arbitrary text encodings into
/// utf-8 to interoperate with standard rust text types
extern crate encoding_rs;

pub mod reader;
// TODO: pub mod writer;

pub const DEFAULT_BUF_SIZE: usize = 4096;
