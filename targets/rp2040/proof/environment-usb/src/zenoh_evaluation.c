/* Finite carrier experiment, NOT a Body, protected Line or durable recorder.
 * Library owns session reconnection; no parallel relay/backoff loop here.
 */
#include <zenoh-pico.h>
#include <stdint.h>
_Static_assert(Z_FEATURE_MULTI_THREAD == 0, "one isolated C worker");
extern int32_t conduit_evaluation_link_up(void);
extern int32_t conduit_evaluation_publish(void *session, const void *key);
extern void conduit_evaluation_event(uint32_t event, int32_t result);
int32_t conduit_evaluation_put(void *session, const void *key, const uint8_t *bytes, size_t len) {
    z_owned_bytes_t payload;
    z_result_t result=z_bytes_copy_from_buf(&payload,bytes,len);
    if(result<0) return result;
    return z_put(z_loan(*(z_owned_session_t *)session),
                 z_loan(*(const z_view_keyexpr_t *)key),z_move(payload),NULL);
}
int32_t conduit_evaluation_run(const char *locator) {
    while(!conduit_evaluation_link_up()) { if(z_sleep_ms(10)<0) return -1; }
    z_owned_config_t config;
    z_result_t result=z_config_default(&config);
    if(result<0) return result;
    result=zp_config_insert(z_loan_mut(config),Z_CONFIG_MODE_KEY,"client");
    if(result>=0) result=zp_config_insert(z_loan_mut(config),Z_CONFIG_CONNECT_KEY,locator);
    if(result<0) { z_drop(z_move(config)); return result; }
    z_owned_session_t session;
    conduit_evaluation_event(1,0);
    result=z_open(&session,z_move(config),NULL);
    conduit_evaluation_event(2,result);
    if(result<0) return result; /* observable initial failure, no hidden retries */
    z_view_keyexpr_t key;
    result=z_view_keyexpr_from_str(&key,"conduit/evaluation/environment");
    if(result<0) { z_drop(z_move(session)); return result; }
    z_clock_t last=z_clock_now();
    bool first=true;
    for(;;) {
        zp_spin_once(z_loan(session));
        if(first || z_clock_elapsed_ms(&last)>=30000) {
            first=false; last=z_clock_now();
            result=conduit_evaluation_publish(&session,&key);
            conduit_evaluation_event(3,result); /* accepted, not delivered/committed */
        }
        if(z_sleep_ms(1)<0) break;
    }
    z_drop(z_move(session));
    return -1;
}
