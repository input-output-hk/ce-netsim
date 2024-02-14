#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <ostream>
#include <new>

enum class SimError : uint32_t {
  /// the function succeed, no error
  Success = 0,
  /// An undefined error
  Undefined = 1,
  /// the function was called with an unexpected null pointer
  NullPointerArgument = 3,
  /// The function is not yet implemented, please report this issue
  /// to maintainers
  NotImplemented,
};

struct SimContext;

struct SimSocket;

extern "C" {

/// Create a new NetSim Context
///
/// This is configured so that messages of type Box<u8> can be shared through
/// the network between nodes.
///
/// # Safety
///
/// This function allocate a pointer upon success and returns the pointer
/// address. Call [`netsim_context_shutdown`] to release the resource.
///
SimError netsim_context_new(SimContext **output);

/// Shutdown a NetSim context and release assets
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
SimError netsim_context_shutdown(SimContext *context);

/// create a new [`SimSocket`] in the given context
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
SimError netsim_context_open(SimContext *context, SimSocket **output);

/// Access the unique dentifier of the [`SimSocket`]
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
SimError netsim_socket_id(SimSocket *socket, SimId *id);

/// Release the new [`SimSocket`] resources
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
SimError netsim_socket_release(SimSocket *socket);

/// Receive a message from the [`SimSocket`]
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
SimError netsim_socket_recv(SimSocket *socket, uint8_t *msg, uint64_t *size, SimId *from);

/// Send a message to the [`SimSocket`]
///
/// # Safety
///
/// The function checks for the context to be a nullpointer before trying
/// to utilise it. However if the value points to a random value then
/// the function may have unexpected behaviour.
///
SimError netsim_socket_send_to(SimSocket *socket, SimId to, uint8_t *msg, uint64_t size);

} // extern "C"
