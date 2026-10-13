# Settled sixteen-read demonstration

`mass-storage-read-producer.conduit` issues sixteen explicit requests for the
pinned read-only LUN0/LBA0 512-byte block. It waits for each storage result before
requesting another transaction. The provider retains its separate BOT state,
immutable tag bank, framing checks and finite transfer budgets. A failed or
contradictory storage result stops the producer.

The producer and provider have distinct exact state Types. The accompanying
specialization recipe binds `ProducerCell`, `ProducerJoin` and
`BotReadProducerPair` to typed instances of the ordinary finite semantic
families. Packaging derives each instance identity from the family, checked
schema and value contract, then adds exact `with` bindings to the packaged
Source. That packaged Source and the immutable recipe must be retained as the
receipt inputs. No provider, device authority or resource possession comes from
an alias.

Native acceptance requires a matching prepared product, sixteen exact block
results with ordinals 0 through 15, sixteen device CBWs with distinct tags 1
through 16, successful status correlation and complete retirement. Compilation
or packaging alone does not prove those requirements. Physical storage and the
fault/cancellation matrix remain separate acceptance work.
