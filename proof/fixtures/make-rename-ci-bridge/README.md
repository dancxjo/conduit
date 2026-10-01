# Make rename CI bridge

These empty packages retain three pre-rename Cargo identities solely for the
trusted stable CI controller that evaluates pull request #4485. They expose no
library, executable, public entrance, or xtask job. The official term and all
real implementations are **make**.

The same controller also fingerprints two retired source paths. Their inert
tombstones live at `products/tour` and
`targets/conduitos/fabrication/xtask/profile.rs`; neither is compiled or exposed.

Remove this fixture after the controller containing the `make` proof graph has
been promoted to `main`.
