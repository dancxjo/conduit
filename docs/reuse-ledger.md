# Reuse Ledger

Treat old code as source material, not inheritance.

| New component | Legacy source | Decision |
|---|---|---|
| Panel lexer | archive branch, old parser module | Rewritten narrowly |
| Bounded cord | old runtime ring implementation | Copied and simplified |
| Source spans | old source document types | Recreated without UI fields |
