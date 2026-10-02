## forms and completion

For a visual companion to the syntax below, browse the generated
[[Form diagrams|Form-diagrams]].

A form is **live by default**:

```conduit
form live-example (
    >> reading: Temperature...
    label: Text >>
) {
    reading >> (. > 30°C ? "hot" : "fine") >> label
}
```

A trailing full stop after the closing brace declares **semantic completion on structural drain**:

```conduit
form finite-example (
    >> input: Text
    output: Text >>
) {
    input >> text/upper >> output
}.
```

## Semantic full stop: `}` versus `}.`

The full stop belongs to the **form as a whole**, not to the sequence of statements inside its back.

Canonical law:

```text
}     structural drain = quiescence; the play remains live for later admitted work
}.    structural drain = semantic fulfillment; the play terminates normally as completed
```

More precisely:

> A trailing `.` on the form declares that when its admitted realization reaches **structural drain under the ordinary scheduler law**, that drain is a sufficient completion witness for the form's authored meaning.

The period does **not** mean “stop executing now,” and it is not an executable statement, gear, cord, effect, cancellation request, or timeout.

It also does **not** mean “terminate whenever nothing happens to be runnable this instant.” Temporary lack of runnable work, an outstanding admitted activation, waiting input/provider work, or any other condition that has not reached the scheduler's actual structural-drain state is not promoted to completion by punctuation.

Without the trailing full stop, the same structural drain is **quiescence**. The exact play remains alive and may continue when later admitted input or activation arrives.

With the trailing full stop, the same structural drain is **semantic completion**. The play may terminate normally with completed disposition.

Thus the punctuation changes checked meaning, plan completion policy, and lifecycle behavior:

```text
form ... { ... }     live form
form ... { ... }.    finite-on-drain form
```

This is intentionally punctuation on the form boundary. Do **not** encode semantic completion as a freestanding `.` statement inside the body.

The historical/current implementation that accepts a standalone body `.` is now a migration target, not canon.

The English keyword `complete` is **not Conduitese** and must be rejected rather than retained as a compatibility alias.

A standalone `...` currently tolerated by old parsing is **not canonical semantic syntax**. It may survive losslessly as authoring trivia/TODO, but it must not acquire continuation, infinity, liveness, completion, or execution meaning.

Provenance: #3939 plus this canonical amendment in #4109.
---

## cords and fore direction: `>>`

`>>` is the **one canonical authored cord/direction token**.

```conduit
source >> transform >> sink

form upper (
    >> input: Text
    output: Text >>
) {
    input >> text/upper >> output
}
```

Where a paired fore is useful:

```conduit
form upper (
    input: Text >> output: Text
) {
    input >> text/upper >> output
}
```

The ordinary operators `>`, `<`, `>=`, and `<=` belong to comparison/constraint mathematics, **not graph carriage**.

Reserve expression shifts:

```text
>>    cord / fore direction
>>>   right shift in pure expressions
<<<   left shift in pure expressions
```

Historical `>` cord source remains historical evidence. **Do not make `>` a current compatibility spelling.**

Provenance: #3967.

---

## Pure one-input expressions

A pure expression has **one current runtime input**, written:

```text
.        whole input
.field   record field
.0       tuple element
```

and may reference immutable lexical/startup values.

Pure expressions are finite and may not hide:

- host calls/effects;
- keep/state ownership;
- a second temporal input;
- waits/suspension;
- clock/random/resource acquisition except as explicit semantic input.

Examples:

```conduit
reading >> (.temperature > 30°C ? "hot" : "fine") >> label
flags >> ((. >>> 8) & 0xff) >> byte
```

## Ternary

Canonical pure value-selection sugar:

```conduit
condition ? yes : no
```

The condition is Boolean, branches unify to one exact finite result type/bound, and only the selected pure branch evaluates.

Ternary is **not graph routing**.

Provenance: #3964, #3969.

---

## Systems-grade scalar expressions

Pure expressions admit exact fixed-width integer types when width is semantic:

~~~text
U8 U16 U32 U64 U128
I8 I16 I32 I64 I128
~~~

Canonical integer literal families include decimal, hexadecimal, binary and octal with optional readability separators:

~~~text
255
0xff
0x8000_0000
0b1000_0001
0o755
~~~

Do not import C integer promotions, host-sized `usize/isize`, target-dependent wrapping, UB, or silent truncation.

Canonical Boolean operators:

~~~text
!a
a && b
a || b
~~~

Canonical bitwise operators for admitted exact integer/bitset types:

~~~text
a & b
a | b
a ^ b
~~~

Shift spelling remains:

~~~text
>>>   right shift
<<<   left shift
>>    Cord / Fore direction, never an expression shift
~~~

Right-shift behavior follows checked signedness. Out-of-range shift counts require one portable checked refusal/result law rather than host-language masking/UB.

Ergonomic number law:

> **Make unsafe ambiguity impossible; do not make machine widths homework.**

Use exact widths where representation is meaning (register/wire/device/ABI). Ordinary arithmetic/counts/quantities should normally use semantic numeric types with planner/compiler-selected safe representation inside exact bounds.

Required precedence, highest to lowest:

~~~text
primary / projection
unary
* / %
+ -
<<< >>>
< <= > >=
== !=
&
^
|
&&
||
?:
>> Cord outside expression islands
~~~

No operator meaning may depend on whitespace.

Provenance: #3964, #3967, #4014.

---

## Immutable locals

Canonical local binding:

```conduit
limit = 30°C
matrix = { a: 1, b: 0, c: 0, d: 1 }
```

`=` declares an immutable lexical relationship. It is not assignment and not temporal sequencing.

No shadowing of startup parameters, ports, gear names, locals, or imported aliases in the same scope.

Local dependency cycles refuse. Runtime-flow capture requires explicit graph/value machinery and must not turn locals into hidden state.

Provenance: #4052.

---

## Exactly-one graph routing

Canonical routing:

```conduit
value >> ? {
    pattern-or-guard: route
    _: otherwise-route
}
```

Example:

```conduit
reading >> ? {
    .temperature > 30°C: . >> alert
    _: _
}
```

Rules:

- routing selects exactly one track;
- `.` is the current selected payload;
- final bare `_:` is the otherwise arm where permitted;
- closed variants require exhaustive arms;
- missing, duplicate, overlapping, unreachable, multiple-otherwise and non-final-otherwise cases refuse;
- unselected effects do not execute;
- graph routing remains distinct from pure ternary.

A route-side bare `_` means **deliberate discard**, once historical payload-placeholder uses have been migrated to `.`.

Provenance: #3938, #4062.

---

## Unary filter sugar: `when`

Canonical unary value filter:

```conduit
temperature >> when(. > limit) >> alarm
```

It lowers to ordinary checked routing plus discard.

It has one current runtime input `.` and may capture immutable lexical/startup values.

It does **not** hide a second temporal input or temporal join.

For flows it preserves modality/terminal law:

```text
T...  -> T...
T...| -> T...|
```

For one `T`, rejecting the value must have an explicit zero-or-one/optional law, naturally `T?`; it must not pretend to remain unconditional `T -> T`.

There is **no pre-release `where` compatibility alias**.

Provenance: #4044.

---

## Source gears

A source gear is ordinary checked work whose activation may originate from an admitted external condition rather than an upstream runtime data cord.

Examples: timer, input device, network receive, sensor observation.

There is **no callback syntax and no second event-loop runtime**. External readiness enters the same bounded scheduler/kernel activation model and obeys ordinary pressure, terminal and lifecycle law.

Provenance: #4060.

---

## Pure semantic kind-call sugar

Reviewed pure semantic kinds may use function-shaped expression sugar:

```conduit
sine = math/sin(angle)
```

This lowers to ordinary semantic kind application. It is not a parser-owned intrinsic or second function runtime.

Eligibility is derived from checked semantic properties. Authors do not write “pinky promise this is pure.”

Effectful, stateful, suspending or authority-bearing kinds refuse in pure expression context.

Provenance: #4053, #4070, #4071.

---
