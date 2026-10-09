"""Independent exact ordering from Fraction and the retained SI reference corpus.

Uses no Rust catalogue or execution output. Temperature equations are shared
with the independent temperature generator; prefix cases use its separately
reviewed arbitrary-precision relative-scale fixture.
"""
import json
import runpy
from fractions import Fraction as F
from pathlib import Path

ROOT = Path(__file__).parent
UNITS = runpy.run_path(str(ROOT / "generate_temperature_reference.py"))["UNITS"]
cases = []


def add(role, left, right, left_value, right_value):
    result = "less" if left_value < right_value else "greater" if left_value > right_value else "equal"
    cases.append({"role": role, "left": left, "right": right, "result": result})


for case in json.loads((ROOT / "quantity_prefix_scales.json").read_text())["cases"]:
    add("quantity", case["source"], "1" + case["base"],
        F(int(case["relative_numerator"]), int(case["relative_denominator"])), F(1))

for role in ("quantity", "difference"):
    values = []
    for unit, (scale, offset) in UNITS.items():
        for number in ("0", "1", "-1", "9", "30", "273.15"):
            values.append((number + unit, F(number) * scale + (offset if role == "quantity" else 0)))
    for index, (left, left_value) in enumerate(values):
        for shift in (0, 1, 27):
            right, right_value = values[(index + shift) % len(values)]
            add(role, left, right, left_value, right_value)

for left, right, left_value, right_value in [
    ("1000mm", "1m", F(1), F(1)),
    ("1m", "0.001km", F(1), F(1)),
    ("1MB", "1MiB", F(10**6), F(2**20)),
    ("1kB", "1000B", F(1000), F(1000)),
    ("1KiB", "1024B", F(1024), F(1024)),
    ("1mW", "1MW", F(1, 1000), F(10**6)),
    ("1kg", "1000g", F(1000), F(1000)),
    ("1mg", "1kg", F(1, 1000), F(1000)),
    ("1000mV", "1V", F(1), F(1)),
    ("1cm²", "100mm²", F(1, 10**4), F(1, 10**4)),
    ("1in", "0.0254m", F(254, 10000), F(254, 10000)),
    ("1min", "60s", F(60), F(60)),
    ("1Qm³", "1qm³", F(10**90), F(1, 10**90)),
    ("-1Qm³", "-1qm³", F(-10**90), F(-1, 10**90)),
    ("0°C", "273.15K", F(27315, 100), F(27315, 100)),
    ("1°F", "0°C", (F(1) - 32) * F(5, 9) + F(27315, 100), F(27315, 100)),
]:
    add("quantity", left, right, left_value, right_value)
add("difference", "9°F", "5K", F(5), F(5))
add("difference", "1m°C", "0.001K", F(1, 1000), F(1, 1000))

lines = ["{", '  "reference": "Python Fraction; independent SI prefix corpus and physical temperature/linear equations; no Rust output",', '  "cases": [']
lines += ["    " + json.dumps(case, ensure_ascii=False) + ("," if index + 1 < len(cases) else "")
          for index, case in enumerate(cases)]
lines += ["  ]", "}"]
(ROOT / "quantity_comparison_reference.json").write_text("\n".join(lines) + "\n")
