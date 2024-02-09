#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef uint64_t Address;

bool send_ffi(Address addr, const ByteBuffer *data);

bool receive_ffi(ByteBuffer *data, Address *addr);
