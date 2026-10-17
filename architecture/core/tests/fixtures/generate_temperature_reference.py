"""Independent SI temperature equations using Python's unbounded Fraction.

Does not import Conduit code or read Rust output. Values are in kelvin, using
Celsius's 273.15 origin and Fahrenheit's (F - 32) * 5/9 + 273.15 equation.
The shared decimal profile's published limits are applied only after division.
"""
import json
from fractions import Fraction as F
from pathlib import Path

UNITS = {
    "K": (F(1), F(0)),
    "mK": (F(1, 1000), F(0)),
    "µK": (F(1, 10**6), F(0)),
    "QK": (F(10**30), F(0)),
    "qK": (F(1, 10**30), F(0)),
    "°C": (F(1), F(27315, 100)),
    "m°C": (F(1, 1000), F(27315, 100)),
    "°F": (F(5, 9), F(27315, 100) - F(32 * 5, 9)),
    "m°F": (F(5, 9000), F(27315, 100) - F(32 * 5, 9)),
}


def project(value):
    divisor = value.denominator
    counts = []
    for factor in (2, 5):
        count = 0
        while divisor % factor == 0:
            divisor //= factor
            count += 1
        counts.append(count)
    if divisor != 1:
        return {"refusal": "inexact"}
    places = max(counts)
    coefficient = value.numerator * 2 ** (places - counts[0]) * 5 ** (places - counts[1])
    exponent = -places
    if coefficient == 0:
        exponent = 0
    else:
        while coefficient % 10 == 0 and exponent < 128:
            coefficient //= 10
            exponent += 1
    if abs(coefficient) >= 10**38 or not -128 <= exponent <= 128:
        return {"refusal": "overflow"}
    return {"coefficient": str(coefficient), "exponent": exponent}


cases = []
for role in ("point", "difference"):
    for source, (scale, offset) in UNITS.items():
        for number in ("0", "1", "-1", "9", "30", "273.15"):
            physical = F(number) * scale + (offset if role == "point" else 0)
            for target, (target_scale, target_offset) in UNITS.items():
                result = (physical - (target_offset if role == "point" else 0)) / target_scale
                cases.append({"role": role, "source": number + source, "target": target,
                              "numerator": str(result.numerator), "denominator": str(result.denominator),
                              **project(result)})

Path(__file__).with_name("quantity_temperature_reference.json").write_text(
    json.dumps({"reference": "SI affine temperature equations; Python Fraction; no Rust output",
                "cases": cases}, ensure_ascii=False, indent=2) + "\n"
)
