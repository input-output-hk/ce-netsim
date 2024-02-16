CC      = gcc
CFLAGS  = -I..
LDFLAGS = -L ${PWD}/../../target/debug/ -lnetsim
RM      = rm
NETSIMLIB = ${PWD}/../../target/debug/libnetsim.a

default: all

all: $(NETSIMLIB) netsim

$(NETSIMLIB):
	cargo build --package netsim-ffi

netsim: main.o
	$(CC) $(CFLAGS) -o $@ $+ $(LDFLAGS)

clean:
	$(RM) -f *.o netsim

%.o: %.c %.h
	$(CC) $(CFLAGS) -c -o $@ $<