Current BOT evidence interpretation
===================================

All 20 retained case artifact identities were revalidated. Cancellation checks pass for hold-in and late-after-cancel on both x86_64 and AArch64. They show original transfer cancellation, no block output, and a device response after cancellation. They do not prove native delivery of that late response.

The historical remaining lists in the original receipts describe their collection time. AArch64 cancellation and Audio guest execution have since earned separate evidence. Standalone checked reset/clear-halt now has native x86 proof in ../native-bot-reset-d2ab8e8a; that does not establish interrupted-transaction reset reentry. Remaining acceptance includes integrated quiescence/reentry, the complete substrate lifecycle matrix, and exact integrated CI.
