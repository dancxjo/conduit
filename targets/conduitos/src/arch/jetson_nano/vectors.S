// Retain the Nano symbol surface while sharing the AArch64 register-save ABI.
.set conduitos_board_irq, conduitos_nano_irq
.set conduitos_board_exception, conduitos_nano_exception
.global conduitos_nano_vectors
.set conduitos_nano_vectors, conduitos_board_vectors
