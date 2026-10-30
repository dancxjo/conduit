/* Finite single-socket platform layout. Header supplies types, never effects. */
#ifndef CONDUIT_EMBASSY_TYPES_H
#define CONDUIT_EMBASSY_TYPES_H
#include <stdint.h>
#include <stddef.h>
typedef uint64_t z_clock_t;
typedef uint64_t z_time_t;
typedef struct { uint32_t slot; uint32_t generation; } _z_sys_net_socket_t;
typedef struct { uint8_t ipv4[4]; uint16_t port; } _z_sys_net_endpoint_t;
#endif
