#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef uint64_t Address;

int32_t add_numbers(int32_t a, int32_t b);

bool send_ffi(Address addr, const ByteBuffer *data);

bool receive_ffi(ByteBuffer *data, Address *addr);
