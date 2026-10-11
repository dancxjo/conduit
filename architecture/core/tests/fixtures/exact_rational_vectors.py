"""Regenerate the independent wide arithmetic oracle with Python Fraction."""
import fractions
import json
import pathlib
import random

rng = random.Random(5392)

def encoded(value):
    n = abs(value.numerator)
    d = value.denominator
    nb = n.to_bytes((n.bit_length() + 7) // 8, 'little')
    db = d.to_bytes((d.bit_length() + 7) // 8, 'little')
    return (bytes([1, value < 0]) + len(nb).to_bytes(2, 'little')
            + len(db).to_bytes(2, 'little') + nb + db).hex()

rows = []
for bits in [64, 127, 255, 511, 1023, 1800]:
    for negative in [False, True]:
        a = fractions.Fraction(rng.getrandbits(bits) * (-1 if negative else 1), rng.getrandbits(bits) | 1)
        b = fractions.Fraction(rng.getrandbits(bits) * (-1 if not negative else 1), rng.getrandbits(bits) | 1)
        rows.append(dict(a=[str(a.numerator), str(a.denominator)],
                         b=[str(b.numerator), str(b.denominator)],
                         add=encoded(a+b), subtract=encoded(a-b),
                         multiply=encoded(a*b), divide=encoded(a/b),
                         order=(a > b) - (a < b)))
pathlib.Path(__file__).with_suffix('.json').write_text(json.dumps(rows, indent=2) + '\n')
