#include <stdint.h>

#include "netsim.h"

static char* MSG = "Hello!";
#define LEN 6

void no_drop(struct Message msg) {
    // Do nothing, we aren't allocating anything
}

int main() {
    SimContext* context = NULL;
    SimError error = SimError_Success;
    
    error = netsim_context_new(&context, no_drop);
    if (error != SimError_Success) {
        goto exit;
    }

    SimSocket* net1;
    SimId net1_id;
    error = netsim_context_open(context, &net1);
    if (error != SimError_Success) { goto cleanup_context; }

    SimSocket* net2;
    SimId net2_id;
    error = netsim_context_open(context, &net2);
    if (error != SimError_Success) { goto cleanup_net1; }

    error = netsim_socket_id(net1, &net1_id);
    if (error != SimError_Success) { goto cleanup; }
    error = netsim_socket_id(net2, &net2_id);
    if (error != SimError_Success) { goto cleanup; }

    struct Message msg = { (uint8_t*) MSG, LEN };
    error = netsim_socket_send_to(net1, net2_id, msg);
    if (error != SimError_Success) { goto cleanup; }

    Message new_msg;
    SimId from;
    error = netsim_socket_recv(net2, &new_msg, &from);
    if (error != SimError_Success) { goto cleanup; }

    if (new_msg.size != LEN) {
        // wrong size
        error = 41;
    }
    if (new_msg.pointer != (uint8_t*)MSG) {
        // wrong message
        error = 42;
    }
    if (from != net1_id) {
        // wrong sender
        error = 44;
    }

cleanup:
    netsim_socket_release(net2);
cleanup_net1:
    netsim_socket_release(net1);
cleanup_context:
    netsim_context_shutdown(context);
exit:
    return(error);
}