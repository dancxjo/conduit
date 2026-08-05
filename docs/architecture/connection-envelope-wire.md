# Connection-envelope wire format

`conduit-wire` format version 1 is deterministic and contains exactly one connection envelope.
All integers are little-endian. Strings are UTF-8 and are length-prefixed in bytes.

| Order | Field | Encoding |
| --- | --- | --- |
| 1 | magic | four bytes: `CNDW` |
| 2 | wire format version | `u8`, currently `1` |
| 3 | Conduit protocol version | `u16` |
| 4 | plan ID | `u16` byte length, then UTF-8 bytes |
| 5 | connection ID | `u16` byte length, then UTF-8 bytes |
| 6 | sequence | `u64` |
| 7 | value kind | `u16` byte length, then UTF-8 bytes |
| 8 | payload | `u32` byte length, then exact payload bytes |

Each identity is limited to 4096 bytes. The caller supplies the accepted payload bound for both
encoding and decoding. Decoding rejects unknown magic, unknown wire-format version, a Conduit
protocol-version mismatch, invalid UTF-8, truncation, oversized identifiers, oversized payload or
frame, and any trailing byte. Concatenated frames therefore require an outer framing transport;
they are not accepted implicitly by this codec.
