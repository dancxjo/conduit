# Explicit local Fore projection

Schema 7 extends the schema 6 native-binding ABI with local external Fore.
Schemas 4–6 retain their bytes and entry points. Old admission functions reject
schema 7. The canonical envelope remains 125 bytes with thirteen finite counts.
Only the explicitly opted-in decoder accepts local Fore; it admits no network
Line records or remote-binding requirements in this profile.

Endpoint tag 10 has schema-specific meaning: schemas 4–6 carry a 250-byte
network binding; schema 7 carries a 53-byte local Fore pin. The latter contains
endpoint and cord ordinals (two little-endian u16s), direction (0 input, 1 output),
the 16-byte front-port identity and a 32-byte resident ABI digest. It carries no
peer, Line, invitation, possession, grant or membership. Reusing a numeric kernel
external endpoint does not turn this local boundary into a network connection.

`AssignedForeRequirement` comes from trusted Make material. The separately
versioned ABI digest covers value kind, canonical checked value contract,
abnormal kind, temporal contract, quantity codec profile, semantic track and
finite item/byte limits. Make lowers the whole sealed Plan independently and
retains the typed Fore contracts for actual value validation. The digest does
not replace typed runtime checks. At most sixteen unique endpoint/cord pins are
accepted, each exactly once; changed, duplicate and missing records refuse.

Both contiguous and borrowed-record entrances verify the current Host/Boot,
complete envelope length/digest/counts, computation inventory and exact Fore
pins without allocation. The constrained table builder separately pins the full
resident layout and matches boundary ordinal, cord and direction. Preparation
runs no Step, grants no caller access and issues no capability.

The environmental output proof exercises a real checked Plan through Make,
compact Fore records and the ordinary fixed kernel, using explicit CPU Root,
pin and authority fixtures. This is distinct from MCU installation, physical
display delivery and continuing Body/Mask/recorder behavior. Those remain the
acceptance work of #6378 and the appliance goal; a decoder test cannot close it.
