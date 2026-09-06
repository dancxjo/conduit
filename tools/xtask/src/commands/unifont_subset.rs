use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Write};

use flate2::read::GzDecoder;

use crate::cli::UnifontSubsetArgs;

const RANGES: &[(u32, u32)] = &[
    (0x0020, 0x007e), // ASCII
    (0x00a0, 0x00ff), // Latin-1 Supplement
    (0x0370, 0x03ff), // Greek and Coptic
    (0x0400, 0x04ff), // Cyrillic
    (0x2190, 0x21ff), // Arrows
    (0x2500, 0x257f), // Box Drawing
    (0x25a0, 0x25ff), // Geometric Shapes
    (0x4e2d, 0x4e2d), // Double-width acceptance glyph
    (0xfffd, 0xfffd), // Explicit replacement glyph
];

pub fn run(args: UnifontSubsetArgs) -> Result<(), Box<dyn std::error::Error>> {
    let input = File::open(&args.input)?;
    let output = File::create(&args.output)?;
    let glyphs = BufReader::new(GzDecoder::new(input));
    let mut subset = BufWriter::new(output);

    write_subset(glyphs, &mut subset)?;
    subset.flush()?;
    Ok(())
}

fn write_subset(mut glyphs: impl BufRead, mut subset: impl Write) -> io::Result<()> {
    let mut line = String::new();
    while glyphs.read_line(&mut line)? != 0 {
        if !line.is_ascii() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unifont input must be ASCII",
            ));
        }

        if let Some((codepoint, _)) = line.split_once(':') {
            let codepoint = u32::from_str_radix(codepoint, 16).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid Unifont codepoint {codepoint:?}: {error}"),
                )
            })?;
            if selected(codepoint) {
                subset.write_all(line.as_bytes())?;
            }
        }
        line.clear();
    }
    Ok(())
}

fn selected(codepoint: u32) -> bool {
    RANGES
        .iter()
        .any(|&(start, end)| start <= codepoint && codepoint <= end)
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor, Write};

    use flate2::{write::GzEncoder, Compression};

    use super::*;

    #[test]
    fn writes_only_the_bounded_patchbay_ranges() {
        let source = b"001F:00\n0020:01\n007E:02\n0100:03\n0370:04\n4E2D:05\nFFFD:06\n";
        let mut output = Vec::new();

        write_subset(Cursor::new(source), &mut output).expect("subset generation succeeds");

        assert_eq!(output, b"0020:01\n007E:02\n0370:04\n4E2D:05\nFFFD:06\n");
    }

    #[test]
    fn rejects_a_malformed_codepoint() {
        let error = write_subset(Cursor::new(b"not-hex:00\n"), Vec::new())
            .expect_err("malformed codepoint must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn reads_gzip_input() {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(b"0041:AA\n0100:BB\n").unwrap();
        let compressed = encoder.finish().unwrap();
        let glyphs = BufReader::new(GzDecoder::new(Cursor::new(compressed)));
        let mut output = Vec::new();

        write_subset(glyphs, &mut output).expect("gzip subset generation succeeds");

        assert_eq!(output, b"0041:AA\n");
    }
}
