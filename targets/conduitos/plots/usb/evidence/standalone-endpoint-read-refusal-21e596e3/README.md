Endpoint-only preparation regression
====================================

The unchanged usb-endpoint-read-call package contains one installed endpoint-read placement and no control placement. The approved Root selects the real external CDC attachment and endpoint 0x81, bulk, maximum packet64. The product kernel21e596e3 refuses before Play with Possession(WrongSelection). No transfer or successful endpoint result is claimed.

The endpoint preparation path currently issues both control and endpoint possession unconditionally. Its control issuer cannot find a control selection in this exact Plan. This failed guest artifact is retained before correction; adding an artificial control placement would not prove the standalone entry.
