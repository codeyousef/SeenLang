use anyhow::{Context, Result};
use object::{write::Object, write::StandardSegment, BinaryFormat, Endianness, ObjectSection};

pub fn embed_build_id(binary: &[u8], build_id: &[u8]) -> Result<Vec<u8>> {
    let mut obj = object::File::parse(binary)
        .context("failed to parse binary for build-id embedding")?
        .to_owned();
    let section = obj.add_section(
        obj.segment_name(StandardSegment::Data).to_vec(),
        b".note.seen.build_id".to_vec(),
        object::SectionKind::Note,
    );
    obj.section_mut(section).set_data(build_id.to_vec(), 4);
    Ok(obj.write()?)
}
