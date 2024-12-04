/**
 * Ffi for netsim
 *
 *
 * Copyright 2024, Input Output HK Ltd
 * Licensed with: Apache-2.0
 */

#ifndef NETSIM_LIBC
#define NETSIM_LIBC

/* Generated with cbindgen:0.26.0 */

/* Warning, this file is autogenerated by cbindgen. Don't modify this manually. */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include "netsim_extra.h"

enum SimError
{
  /**
   * the function succeed, no error
   */
  SimError_Success = 0,
  /**
   * An undefined error
   */
  SimError_Undefined = 1,
  /**
   * the function was called with an unexpected null pointer
   */
  SimError_NullPointerArgument = 3,
  /**
   * The function is not yet implemented, please report this issue
   * to maintainers
   */
  SimError_NotImplemented = 4,
  /**
   * This indicates it's time to release the socket
   */
  SimError_SocketDisconnected = 5,
};
typedef uint32_t SimError;

typedef struct SimContext SimContext;

typedef struct SimSocket SimSocket;

typedef struct Message
{
  void *pointer;
  uint64_t size;
} Message;

/**
 * Create a new NetSim Context
 *
 * This is configured so that messages of type Box<u8> can be shared through
 * the network between nodes.
 *
 * # Safety
 *
 * This function allocate a pointer upon success and returns the pointer
 * address. Call [`netsim_context_shutdown`] to release the resource.
 *
 */
SimError netsim_context_new(struct SimContext **output,
                            void (*on_drop)(struct Message));

/**
 * create a new [`SimSocket`] in the given context
 *
 * # Safety
 *
 * The function checks for the context to be a nullpointer before trying
 * to utilise it. However if the value points to a random value then
 * the function may have unexpected behaviour.
 *
 */
SimError netsim_context_open(struct SimContext *context,
                             struct SimSocket **output);

/**
 * Shutdown a NetSim context and release assets
 *
 * # Safety
 *
 * The function checks for the context to be a nullpointer before trying
 * to utilise it. However if the value points to a random value then
 * the function may have unexpected behaviour.
 *
 */
SimError netsim_context_shutdown(struct SimContext *context);

/**
 * Access the unique identifier of the [`SimSocket`]
 *
 * # Safety
 *
 * The function checks for the context to be a nullpointer before trying
 * to utilise it. However if the value points to a random value then
 * the function may have unexpected behaviour.
 *
 */
SimError netsim_socket_id(struct SimSocket *socket, NodeId *id);

/**
 * Receive a message from the [`SimSocket`]
 *
 * On success the function populate the pointed value `msg` with the
 * received message. As well as `from` with the sender of the message.
 *
 * # Safety
 *
 * The function checks the parameters to be non null before trying
 * to utilise it. However if the pointers point to a random memory then
 * the function may have unexpected behaviour. Same for `msg` and `from`
 *
 */
SimError netsim_socket_recv(struct SimSocket *socket,
                            struct Message *msg,
                            NodeId *from);

/**
 * Release the new [`SimSocket`] resources
 *
 * # Safety
 *
 * The function checks for the context to be a nullpointer before trying
 * to utilise it. However if the value points to a random value then
 * the function may have unexpected behaviour.
 *
 */
SimError netsim_socket_release(struct SimSocket *socket);

/**
 * Send a message to the [`SimSocket`]
 *
 * # Safety
 *
 * The function checks for the context to be a nullpointer before trying
 * to utilise it. However if the value points to a random value then
 * the function may have unexpected behaviour.
 * This function returns immediately.
 *
 */
SimError netsim_socket_send_to(struct SimSocket *socket,
                               NodeId to,
                               struct Message msg);

#endif /* NETSIM_LIBC */
