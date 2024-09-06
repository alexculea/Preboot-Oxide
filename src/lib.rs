#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate phf;
#[macro_use]
extern crate clap;

pub mod conf;
pub mod dhcp;
pub mod tftp;
pub mod util;
pub mod cli;

pub type Result<T> = anyhow::Result<T, anyhow::Error>;
