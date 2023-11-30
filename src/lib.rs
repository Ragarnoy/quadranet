#![cfg_attr(not(test), no_std)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

extern crate alloc;

pub mod device;
pub mod message;
pub mod route;
