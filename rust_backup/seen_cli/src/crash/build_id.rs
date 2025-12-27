use object::{Object, ObjectSection};

pub fn extract_build_id(data: &[u8]) -> Result<Option<Vec<u8>>, object::Error> {
    let file = object::File::parse(data)?;
    for section in file.sections() {
        if let Ok(name) = section.name() {
            if name == ".note.seen.build_id" {
                let bytes = section.data()?.to_vec();
                return Ok(Some(bytes));
            }
        }
    }
    Ok(None)
}
