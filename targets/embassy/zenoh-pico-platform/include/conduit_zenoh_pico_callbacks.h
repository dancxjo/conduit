/* External candidate ABI. These symbols MUST be supplied by real Rust resources.
 * No callback implementation, allocator, clock, entropy or network is supplied here.
 * Called only from the admitted C worker; Embassy socket remains on its owner core.
 */
#ifndef CONDUIT_Z_CP_CALLBACKS_H
#define CONDUIT_Z_CP_CALLBACKS_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifndef CONDUIT_Z_ALLOC_ALIGNMENT
#error "Supply the actual Rust C arena alignment"
#endif
#if defined(CONDUIT_Z_ALLOC_ALIGNMENT)
_Static_assert(CONDUIT_Z_ALLOC_ALIGNMENT >= _Alignof(max_align_t), "C arena max_align_t");
_Static_assert((CONDUIT_Z_ALLOC_ALIGNMENT & (CONDUIT_Z_ALLOC_ALIGNMENT-1)) == 0, "C arena power-of-two alignment");
#endif
/* Arena: max_align_t alignment; finite admitted storage; NULL on exhaustion.
 * realloc failure preserves original allocation; free(NULL) is valid. */
void *conduit_z_alloc(size_t size);
void *conduit_z_realloc(void *ptr, size_t size);
void conduit_z_free(void *ptr);
/* Boot-local monotonic microseconds, never Unix time; must not move backwards. */
uint64_t conduit_z_boot_us(void);
/* Bounded worker sleep/yield, not a sampler-core busy wait. 0 success, <0 refusal. */
int32_t conduit_z_sleep_us(uint64_t duration_us);
/* Must fill ALL bytes from actual hardware/approved RNG before returning.
 * Upstream has no error result: unavailable entropy must fail-stop, never use zeros. */
void conduit_z_random_fill(void *dst, size_t len);
/* 0 only with real wall-time provenance; <0 when unknown. nanos < 1e9. */
int32_t conduit_z_wall_time(uint32_t *secs, uint32_t *nanos);
/* Socket callbacks have finite deadlines including bridge queue/response wait.
 * 0 timeout means poll, never infinite. A callback must not retain a borrowed
 * buffer after returning. Expired operations must not later touch that buffer.
 * Every effect validates slot AND nonzero generation; stale handles refuse.
 * connect publishes a new generation only after actual connect, no implicit retry.
 * Only one connection is admitted by this profile; occupied slot refuses. */
int32_t conduit_z_tcp_connect(const uint8_t ipv4[4], uint16_t port, uint32_t timeout_ms,
                             uint32_t *slot, uint32_t *generation);
/* >=0 transferred bytes <=len; read0 EOF; <0 timeout/fault/refusal.
 * A successful write means bytes accepted by transport, NEVER durable ACK. */
intptr_t conduit_z_tcp_read(uint32_t slot, uint32_t generation, uint8_t *dst, size_t len, uint32_t timeout_ms);
/* Known prefix, bounded by timeout_ms even when reads are nonblocking. The PAL
 * assembles a complete streamed write under one absolute deadline. */
intptr_t conduit_z_tcp_write(uint32_t slot, uint32_t generation, const uint8_t *src, size_t len, uint32_t timeout_ms);
/* close success requires actual owner retirement. On finite deadline/fault,
 * return negative and leave generation quarantined until actual retirement;
 * never report reusable backing prematurely. No borrowed caller buffer may
 * remain referenced after return, including on failure. */
int32_t conduit_z_tcp_close(uint32_t slot, uint32_t generation);
/* ready: 1 readable (including EOF), 0 deadline/no readiness, <0 fault. */
int32_t conduit_z_tcp_ready(uint32_t slot, uint32_t generation, uint32_t timeout_ms);
int32_t conduit_z_tcp_set_blocking(uint32_t slot, uint32_t generation, bool blocking);
/* Actual connected local/remote IPv4:port, NUL terminated; fail if unavailable or
 * insufficient capacity. Never fabricate local endpoints from configured peer. */
int32_t conduit_z_tcp_endpoints(uint32_t slot, uint32_t generation,
                               char *local, size_t local_len, char *remote, size_t remote_len);
#endif
