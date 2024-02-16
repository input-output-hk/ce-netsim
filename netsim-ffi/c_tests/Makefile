CC      = gcc
CFLAGS  = -I..
LDFLAGS = -L ${PWD}/../../target/debug/ -lnetsim
RM      = rm
NETSIMLIBS = ${PWD}/../../target/debug/libnetsim.a ${PWD}/../../target/debug/libnetsim.so

default: all

all: $(NETSIMLIBS) netsim

$(NETSIMLIBS):
	cargo build --package netsim-ffi

netsim: main.o
	$(CC) $(CFLAGS) -o $@ $+ $(LDFLAGS)

clean:
	$(RM) -f *.o netsim

%.o: %.c %.h
	$(CC) $(CFLAGS) -c -o $@ $<