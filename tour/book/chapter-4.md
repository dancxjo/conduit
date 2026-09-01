# Use two Hosts

One Form can use more than one Host. Here one browser Host produces the text and another presents it.

```conduit run two-host
form hello-across {
    message: text/literal("hello across one Cord")
    show: presentation/text
    message > show
}
```

Run it. The Form does not split into a client program and a server program; Conduit places its Gears and carries the typed value across one admitted Line.

# Plans and Plays

The Form is the durable question. A **Plan** is one exact answer for the Hosts and Lines available now. A **Play** executes that answer.

```conduit run two-host plan
form hello-across {
    message: text/literal("hello across one Cord")
    show: presentation/text
    message > show
}
```

Run the same Form again. The friendly view shows the two placements and their Cord. Exact identities and the complete Plan remain available under **Inspect exact evidence**.
