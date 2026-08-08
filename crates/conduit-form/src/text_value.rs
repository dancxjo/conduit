pub(crate) fn parse_quoted_text(source: &str) -> Option<String> {
    let body = source.strip_prefix('"')?.strip_suffix('"')?;
    let mut decoded = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(character) = chars.next() {
        if character != '\\' {
            decoded.push(character);
            continue;
        }
        decoded.push(match chars.next()? {
            '"' => '"',
            '\\' => '\\',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            _ => return None,
        });
    }
    Some(decoded)
}
