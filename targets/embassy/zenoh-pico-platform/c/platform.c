/* External Zenoh-Pico1.10.2 PAL candidate. No OS, fake I/O or callback definitions.
 * Single worker / outbound IPv4 TCP only. Requires explicit real Rust callbacks.
 */
#include "conduit_zenoh_pico_callbacks.h"
#include <limits.h>
#include <string.h>
#include "zenoh-pico/config.h"
#include "zenoh-pico/system/platform.h"
#include "zenoh-pico/link/transport/tcp.h"
#include "zenoh-pico/link/transport/socket.h"

void *z_malloc(size_t n) { return conduit_z_alloc(n); }
void *z_realloc(void *p, size_t n) { return conduit_z_realloc(p, n); }
void z_free(void *p) { conduit_z_free(p); }
void z_random_fill(void *p, size_t n) { conduit_z_random_fill(p, n); }
uint8_t z_random_u8(void) { uint8_t v; z_random_fill(&v, sizeof v); return v; }
uint16_t z_random_u16(void) { uint16_t v; z_random_fill(&v, sizeof v); return v; }
uint32_t z_random_u32(void) { uint32_t v; z_random_fill(&v, sizeof v); return v; }
uint64_t z_random_u64(void) { uint64_t v; z_random_fill(&v, sizeof v); return v; }
static uint64_t add_sat(uint64_t a, uint64_t b) { return b > UINT64_MAX-a ? UINT64_MAX : a+b; }
static uint64_t mul_sat(uint64_t a, uint64_t b) { return a > UINT64_MAX/b ? UINT64_MAX : a*b; }
static unsigned long elapsed(uint64_t a, uint64_t b, uint64_t divisor) {
    uint64_t v = a > b ? (a-b)/divisor : 0;
    return v > ULONG_MAX ? ULONG_MAX : (unsigned long)v;
}
z_clock_t z_clock_now(void) { return conduit_z_boot_us(); }
unsigned long zp_clock_elapsed_us_since(z_clock_t *a,z_clock_t *b) { return elapsed(*a,*b,1); }
unsigned long zp_clock_elapsed_ms_since(z_clock_t *a,z_clock_t *b) { return elapsed(*a,*b,1000); }
unsigned long zp_clock_elapsed_s_since(z_clock_t *a,z_clock_t *b) { return elapsed(*a,*b,1000000); }
unsigned long z_clock_elapsed_us(z_clock_t *a) { return elapsed(conduit_z_boot_us(),*a,1); }
unsigned long z_clock_elapsed_ms(z_clock_t *a) { return elapsed(conduit_z_boot_us(),*a,1000); }
unsigned long z_clock_elapsed_s(z_clock_t *a) { return elapsed(conduit_z_boot_us(),*a,1000000); }
void z_clock_advance_us(z_clock_t *a,unsigned long n) { *a=add_sat(*a,n); }
void z_clock_advance_ms(z_clock_t *a,unsigned long n) { *a=add_sat(*a,mul_sat(n,1000)); }
void z_clock_advance_s(z_clock_t *a,unsigned long n) { *a=add_sat(*a,mul_sat(n,1000000)); }
/* Opaque elapsed-time token only. The epoch entrance below is deliberately separate. */
z_time_t z_time_now(void) { return conduit_z_boot_us(); }
unsigned long z_time_elapsed_us(z_time_t *a) { return elapsed(conduit_z_boot_us(),*a,1); }
unsigned long z_time_elapsed_ms(z_time_t *a) { return elapsed(conduit_z_boot_us(),*a,1000); }
unsigned long z_time_elapsed_s(z_time_t *a) { return elapsed(conduit_z_boot_us(),*a,1000000); }
const char *z_time_now_as_str(char *const buf,unsigned long len) {
    if (buf && len) buf[0]='\0';
    return NULL; /* No fabricated UTC, even a boot-relative printable timestamp. */
}
z_result_t _z_get_time_since_epoch(_z_time_since_epoch *t) {
    uint32_t secs=0,nanos=0;
    if (!t || conduit_z_wall_time(&secs,&nanos)!=0 || nanos>=1000000000U)
        return _Z_ERR_TIMESTAMP_GENERATION_FAILED;
    t->secs=secs; t->nanos=nanos; return _Z_RES_OK;
}
z_result_t z_sleep_us(size_t n) { return conduit_z_sleep_us(n)==0 ? _Z_RES_OK : _Z_ERR_SYSTEM_GENERIC; }
z_result_t z_sleep_ms(size_t n) {
    #if SIZE_MAX > UINT32_MAX
    if (n > UINT64_MAX/1000) return _Z_ERR_OVERFLOW;
    #endif
    return conduit_z_sleep_us((uint64_t)n*1000)==0 ? _Z_RES_OK : _Z_ERR_SYSTEM_GENERIC;
}
z_result_t z_sleep_s(size_t n) {
    #if SIZE_MAX > UINT32_MAX
    if (n > UINT64_MAX/1000000) return _Z_ERR_OVERFLOW;
    #endif
    return conduit_z_sleep_us((uint64_t)n*1000000)==0 ? _Z_RES_OK : _Z_ERR_SYSTEM_GENERIC;
}
/* Strict bounded decimal parser; reject signs, spaces, DNS, IPv6 and leading zeros. */
static bool decimal(const char **p,unsigned max,unsigned *out) {
    const char *s=*p; unsigned v=0,count=0;
    while (*s>='0' && *s<='9') {
        unsigned digit=(unsigned)(*s-'0');
        if (++count>5 || v>(max-digit)/10 || (count>1 && **p=='0')) return false;
        v=v*10+digit; ++s;
    }
    if (!count) return false;
    *p=s; *out=v; return true;
}
z_result_t _z_tcp_endpoint_init(_z_sys_net_endpoint_t *ep,const char *host,const char *port) {
    if (!ep || !host || !port) return _Z_ERR_CONFIG_LOCATOR_INVALID;
    _z_sys_net_endpoint_t candidate={0}; const char *p=host; unsigned v=0;
    for (unsigned i=0;i<4;i++) {
        if (!decimal(&p,255,&v)) return _Z_ERR_CONFIG_LOCATOR_INVALID;
        candidate.ipv4[i]=(uint8_t)v;
        if (i<3 && *p++!='.') return _Z_ERR_CONFIG_LOCATOR_INVALID;
    }
    if (*p) return _Z_ERR_CONFIG_LOCATOR_INVALID;
    p=port;
    if (!decimal(&p,65535,&v) || *p || !v) return _Z_ERR_CONFIG_LOCATOR_INVALID;
    candidate.port=(uint16_t)v; *ep=candidate; return _Z_RES_OK;
}
void _z_tcp_endpoint_clear(_z_sys_net_endpoint_t *ep) { if(ep) memset(ep,0,sizeof *ep); }
static bool valid(_z_sys_net_socket_t s) { return s.generation!=0; }
z_result_t _z_tcp_open(_z_sys_net_socket_t *s,const _z_sys_net_endpoint_t ep,uint32_t timeout) {
    if (!s || !ep.port) return _Z_ERR_CONFIG_LOCATOR_INVALID;
    *s=(_z_sys_net_socket_t){0};
    uint32_t slot=0,generation=0;
    if (timeout>Z_TRANSPORT_CONNECT_TIMEOUT) timeout=Z_TRANSPORT_CONNECT_TIMEOUT;
    if (conduit_z_tcp_connect(ep.ipv4,ep.port,timeout,&slot,&generation)!=0 || !generation)
        return _Z_ERR_TRANSPORT_OPEN_FAILED;
    s->slot=slot; s->generation=generation; return _Z_RES_OK;
}
z_result_t _z_tcp_listen(_z_sys_net_socket_t *s,const _z_sys_net_endpoint_t ep) {
    (void)s; (void)ep; return _Z_ERR_TRANSPORT_NOT_AVAILABLE;
}
z_result_t _z_tcp_accept(const _z_sys_net_socket_t *s,_z_sys_net_socket_t *out) {
    (void)s; (void)out; return _Z_ERR_TRANSPORT_NOT_AVAILABLE;
}
void _z_socket_close(_z_sys_net_socket_t *s) {
    if (s && valid(*s)) { (void)conduit_z_tcp_close(s->slot,s->generation); s->generation=0; }
}
void _z_tcp_close(_z_sys_net_socket_t *s) { _z_socket_close(s); }
z_result_t _z_socket_set_blocking(const _z_sys_net_socket_t *s,bool blocking) {
    return s && valid(*s) && conduit_z_tcp_set_blocking(s->slot,s->generation,blocking)==0
        ? _Z_RES_OK : _Z_ERR_SYSTEM_GENERIC;
}
static size_t read_for(_z_sys_net_socket_t s,uint8_t *p,size_t n,uint32_t timeout) {
    if (!n) return 0;
    if (!valid(s) || !p || n>INTPTR_MAX) return SIZE_MAX;
    intptr_t r=conduit_z_tcp_read(s.slot,s.generation,p,n,timeout);
    return r<0 || (size_t)r>n ? SIZE_MAX : (size_t)r;
}
size_t _z_tcp_read(_z_sys_net_socket_t s,uint8_t *p,size_t n) { return read_for(s,p,n,Z_CONFIG_SOCKET_TIMEOUT); }
size_t _z_tcp_read_exact(_z_sys_net_socket_t s,uint8_t *p,size_t n) {
    if (!n) return 0;
    if (!p || n>INTPTR_MAX) return SIZE_MAX;
    uint64_t deadline=add_sat(conduit_z_boot_us(),(uint64_t)Z_CONFIG_SOCKET_TIMEOUT*1000);
    size_t done=0;
    while (done<n) {
        uint64_t now=conduit_z_boot_us();
        if (now>=deadline) return SIZE_MAX;
        uint32_t remaining=(uint32_t)((deadline-now+999)/1000);
        size_t got=read_for(s,p+done,n-done,remaining);
        if (got==SIZE_MAX || got==0) return got;
        done+=got;
    }
    return done;
}
size_t _z_tcp_write(_z_sys_net_socket_t s,const uint8_t *p,size_t n) {
    if (!n) return 0;
    if (!valid(s) || !p || n>INTPTR_MAX) return SIZE_MAX;
    intptr_t r=conduit_z_tcp_write(s.slot,s.generation,p,n,Z_CONFIG_SOCKET_TIMEOUT);
    return r<0 || (size_t)r>n ? SIZE_MAX : (size_t)r;
}
z_result_t _z_socket_wait_readable(_z_socket_wait_iter_t *iter,uint32_t timeout) {
    if (!iter) return _Z_ERR_INVALID;
    _z_sys_net_socket_t selected={0}; unsigned count=0;
    _z_socket_wait_iter_reset(iter);
    while (_z_socket_wait_iter_next(iter)) {
        _z_socket_wait_iter_set_ready(iter,false);
        const _z_sys_net_socket_t *s=_z_socket_wait_iter_get_socket(iter);
        if (!s || !valid(*s)) return _Z_ERR_INVALID;
        selected=*s; if (++count>1) return _Z_ERR_TRANSPORT_NO_SPACE;
    }
    if (!count) return _Z_RES_OK;
    if (timeout>Z_CONFIG_SOCKET_TIMEOUT) timeout=Z_CONFIG_SOCKET_TIMEOUT;
    int32_t ready=conduit_z_tcp_ready(selected.slot,selected.generation,timeout);
    if (ready<0 || ready>1) return _Z_ERR_SYSTEM_GENERIC;
    _z_socket_wait_iter_reset(iter);
    if (!_z_socket_wait_iter_next(iter)) return _Z_ERR_INVALID;
    _z_socket_wait_iter_set_ready(iter,ready==1);
    return ready==1 ? _Z_RES_OK : _Z_NO_DATA_PROCESSED;
}
z_result_t _z_socket_get_endpoints(const _z_sys_net_socket_t *s,char *local,size_t nl,char *remote,size_t nr) {
    return s && valid(*s) && conduit_z_tcp_endpoints(s->slot,s->generation,local,nl,remote,nr)==0
        ? _Z_RES_OK : _Z_ERR_SYSTEM_GENERIC;
}
/* Allocation-free IPv4 endpoint formatter, atomic output on insufficient capacity. */
z_result_t _z_ip_port_to_endpoint(const uint8_t *ip,size_t n,uint16_t port,char *dst,size_t cap) {
    if (!ip || n!=4 || !dst || !port) return _Z_ERR_INVALID;
    char tmp[22]; size_t used=0;
    for (unsigned i=0;i<5;i++) {
        unsigned v=i<4 ? ip[i] : port; char digits[5]; size_t len=0;
        do { digits[len++]=(char)('0'+v%10); v/=10; } while(v);
        while(len) tmp[used++]=digits[--len];
        if (i<4) tmp[used++]=i==3 ? ':' : '.';
    }
    if (cap<=used) return _Z_ERR_OVERFLOW;
    memcpy(dst,tmp,used); dst[used]='\0'; return _Z_RES_OK;
}
