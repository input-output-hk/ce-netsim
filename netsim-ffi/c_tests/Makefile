CC      = gcc
CFLAGS  = -o1 -I.. ${PWD}/../../target/debug/libnetsim.a
RM      = rm

default: all

all: netsim

netsim: main.c
	$(CC) $(CFLAGS) -o netsim main.c

clean:
	$(RM) netsim