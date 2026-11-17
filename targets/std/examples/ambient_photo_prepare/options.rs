pub struct Options<'a> {
    pub date: &'a str,
    pub output: &'a str,
    pub source: &'a str,
    pub manifest: Option<&'a str>,
    pub checksum: Option<&'a str>,
}
pub fn parse(args: &[String]) -> Result<Options<'_>, String> {
    if args.len() % 2 != 0 || args.len() > 10 {
        return Err("ambient photo options must be finite named pairs".into());
    }
    let mut fields = [None; 5];
    for pair in args.chunks_exact(2) {
        let index = match pair[0].as_str() {
            "--date" => 0,
            "--output" => 1,
            "--source" => 2,
            "--manifest" => 3,
            "--manifest-sha256" => 4,
            _ => return Err("unknown ambient photo option".into()),
        };
        if fields[index].replace(pair[1].as_str()).is_some() {
            return Err("duplicate ambient photo option".into());
        }
    }
    let date = fields[0].ok_or("explicit --date required")?;
    let output = fields[1].ok_or("explicit --output required")?;
    let source = fields[2].unwrap_or("apod-api");
    if date.len() != 10
        || date.as_bytes()[4] != b'-'
        || date.as_bytes()[7] != b'-'
        || !date
            .bytes()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit())
        || output.is_empty()
        || output.len() > 2048
    {
        return Err("ambient photo date/output bound".into());
    }
    match source {
        "apod-api" if fields[3].is_none() && fields[4].is_none() => {}
        "candidate-manifest"
            if fields[3].is_some_and(|p| !p.is_empty() && p.len() <= 2048)
                && fields[4]
                    .is_some_and(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())) => {
        }
        _ => return Err(
            "choose apod-api, or candidate-manifest with explicit --manifest and --manifest-sha256"
                .into(),
        ),
    }
    Ok(Options {
        date,
        output,
        source,
        manifest: fields[3],
        checksum: fields[4],
    })
}
