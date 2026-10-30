/* Real target PAL linked below; only effect callbacks are deterministic fixtures.
 * This is CPU boundary evidence, never transport delivery or durable ACK proof. */
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "zenoh-pico/link/transport/tcp.h"
#include "conduit_zenoh_pico_callbacks.h"
struct action { intptr_t accepted; uint64_t elapsed_us; };
static const uint8_t binary[] = {0,255,128,0,19,9};
static struct action actions[8];
static size_t action_count, calls, closes, offset;
static uint64_t now_us, started_us;
static int32_t close_result;
static bool quarantined;
static uint32_t budgets[8];
static uint8_t copied[sizeof binary];
uint64_t conduit_z_boot_us(void) { return now_us; }
intptr_t conduit_z_tcp_write(uint32_t slot,uint32_t generation,const uint8_t *src,size_t len,uint32_t timeout) {
    assert(slot==0 && generation==7);
    assert(calls<action_count);
    assert(src==binary+offset && len==sizeof binary-offset);
    assert(timeout>0 && timeout<=Z_CONFIG_SOCKET_TIMEOUT);
    uint64_t deadline=started_us>UINT64_MAX-(uint64_t)Z_CONFIG_SOCKET_TIMEOUT*1000
        ? UINT64_MAX:started_us+(uint64_t)Z_CONFIG_SOCKET_TIMEOUT*1000;
    assert(now_us<deadline);
    uint64_t remaining=deadline-now_us;
    uint32_t remaining_ms=(uint32_t)(remaining/1000+(remaining%1000!=0));
    if(remaining_ms>Z_CONFIG_SOCKET_TIMEOUT) remaining_ms=Z_CONFIG_SOCKET_TIMEOUT;
    assert(timeout==remaining_ms);
    budgets[calls]=timeout;
    struct action a=actions[calls++];
    assert(a.elapsed_us<=(uint64_t)timeout*1000);
    now_us=UINT64_MAX-now_us<a.elapsed_us ? UINT64_MAX:now_us+a.elapsed_us;
    if(a.accepted>0 && (size_t)a.accepted<=len) {
        memcpy(copied+offset,src,(size_t)a.accepted);offset+=(size_t)a.accepted;
    }
    return a.accepted;
}
int32_t conduit_z_tcp_close(uint32_t slot,uint32_t generation) {
    assert(slot==0 && generation==7);++closes;
    quarantined=close_result<0;
    return close_result;
}
static void reset(uint64_t start) {
    memset(actions,0,sizeof actions);memset(copied,42,sizeof copied);memset(budgets,0,sizeof budgets);
    action_count=calls=closes=offset=0;now_us=started_us=start;close_result=0;quarantined=false;
}
static void action(intptr_t accepted,uint64_t elapsed_us) {assert(action_count<8);actions[action_count++]=(struct action){accepted,elapsed_us};}
static _z_sys_net_socket_t socket_handle(void) {return (_z_sys_net_socket_t){.slot=0,.generation=7};}
static void exact_partial_offsets_and_single_budget(void) {
    reset(1000);action(2,20000);action(1,30000);action(3,10000);
    assert(_z_tcp_write(socket_handle(),binary,sizeof binary)==sizeof binary);
    assert(calls==3 && closes==0 && offset==sizeof binary);
    assert(memcmp(copied,binary,sizeof binary)==0);
    assert(budgets[0]==100 && budgets[1]==80 && budgets[2]==50);
}
static void absolute_deadline_never_restarts_per_chunk(void) {
    reset(0);action(1,60000);action(1,40000);action(4,0);
    assert(_z_tcp_write(socket_handle(),binary,sizeof binary)==SIZE_MAX);
    assert(calls==2 && closes==1 && offset==2 && now_us==100000);
    assert(budgets[0]==100 && budgets[1]==40);
}
static void zero_error_and_overcount_retire_once(void) {
    for(unsigned partial=0;partial<2;partial++) {
        for(unsigned failure=0;failure<3;failure++) {
            reset(0);close_result=-1;
            if(partial) action(2,1000);
            action(failure==0 ? 0 : failure==1 ? -1 : 99,1000);
            assert(_z_tcp_write(socket_handle(),binary,sizeof binary)==SIZE_MAX);
            assert(calls==partial+1 && closes==1 && offset==partial*2);
            assert(quarantined); /* fixture close refusal; PAL must still fail */
        }
    }
}
static void saturated_deadline_still_retires_partial_write(void) {
    reset(UINT64_MAX-50000);action(1,50000);action(5,0);
    assert(_z_tcp_write(socket_handle(),binary,sizeof binary)==SIZE_MAX);
    assert(calls==1 && closes==1 && offset==1 && now_us==UINT64_MAX);
    assert(budgets[0]==50);
}
static void invalid_extents_refuse_before_effects(void) {
    reset(0);
    assert(_z_tcp_write(socket_handle(),NULL,1)==SIZE_MAX);
    assert(_z_tcp_write(socket_handle(),binary,SIZE_MAX)==SIZE_MAX);
    assert(_z_tcp_write((_z_sys_net_socket_t){0},binary,sizeof binary)==SIZE_MAX);
    assert(_z_tcp_write((_z_sys_net_socket_t){.slot=1,.generation=1},binary,sizeof binary)==SIZE_MAX);
    assert(calls==0 && closes==0);
    assert(_z_tcp_write(socket_handle(),NULL,0)==0);
    assert(calls==0 && closes==0);
}
int main(void) {
    exact_partial_offsets_and_single_budget();
    absolute_deadline_never_restarts_per_chunk();
    zero_error_and_overcount_retire_once();
    saturated_deadline_still_retires_partial_write();
    invalid_extents_refuse_before_effects();
    puts("actual PAL full-write fixture:5 scenarios PASS (6 zero/error/overcount variants)");
    return 0;
}
