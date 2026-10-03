//! Read only newline-terminated guest records from a growing emulator transcript.

pub(super) fn complete_json_line<'a>(transcript: &'a str, prefix: &str) -> Option<&'a str> {
    transcript.split_inclusive('\n').find_map(|record| {
        let line = record.strip_suffix('\n')?.trim_end_matches('\r');
        let offset = line.find(prefix)?;
        let json = &line[offset + prefix.len()..];
        json.ends_with('}').then_some(json)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_object_boundary_does_not_complete_a_growing_record() {
        let prefix = "CONDUIT_OBSERVATORY_SNAPSHOT ";
        let partial = "firmware\0CONDUIT_OBSERVATORY_SNAPSHOT {\"host\":{\"id\":1}";
        assert_eq!(complete_json_line(partial, prefix), None);
        assert_eq!(
            complete_json_line(&format!("{partial},\"play\":2}}\n"), prefix),
            Some("{\"host\":{\"id\":1},\"play\":2}")
        );
    }
}
