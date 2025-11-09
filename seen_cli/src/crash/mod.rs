#[link_section = ".note.seen.build_id"]
#[used]
static BUILD_ID_NOTE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/build_id.note"));

pub mod build_id;
