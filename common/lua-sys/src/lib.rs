#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

include!(concat!(env!("OUT_DIR"), "/bindgen.h.rs"));

#[link(name = "lua")]
extern "C"
{
}
