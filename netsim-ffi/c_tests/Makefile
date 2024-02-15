CC      = gcc
CFLAGS  = -o1 -I.. -L ${PWD}/../../target/debug/ -lnetsim
RM      = rm

default: all

all: netsim

netsim: main.c
	$(CC) $(CFLAGS) -o netsim main.c

clean:
	$(RM) netsim