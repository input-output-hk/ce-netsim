// TODO: move the SimId to the header: netsim.h
#include <stdint.h>
typedef uint64_t SimId;

#include "netsim.h"

#define MSG 42

int main() {
    SimContext* context = NULL;
    SimError error = SimError_Success;
    
    error = netsim_context_new(&context);
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

    uint8_t msg[1] = { MSG };
    error = netsim_socket_send_to(net1, net2_id, msg, 1);
    if (error != SimError_Success) { goto cleanup; }

    uint8_t buffer[2] = { 1, 2 };
    uint64_t size = 2;
    SimId from;
    error = netsim_socket_recv(net2, buffer, &size, &from);
    if (error != SimError_Success) { goto cleanup; }

    if (size != 1) {
        // wrong size
        error = 42;
    }
    if (buffer[0] != MSG) {
        // wrong message
        error = 43;
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