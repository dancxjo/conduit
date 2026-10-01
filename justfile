# Optional thin conveniences for Conduit's two supported entrances.
conduit *args:
    conduit {{ args }}

xtask *args:
    cargo xtask {{ args }}

integrate:
    cargo xtask integrate

check *args:
    cargo xtask check {{ args }}
