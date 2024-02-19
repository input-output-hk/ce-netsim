package helper;

import jnr.ffi.util.EnumMapper;

public enum SimError implements EnumMapper.IntegerEnum {
    Success(0),

    /// An undefined error
    Undefined(1),

    /// the function was called with an unexpected null pointer
    NullPointerArgument(3),

    /// The function is not yet implemented, please report this issue
    /// to maintainers
    NotImplemented(4),

    SocketDisconnected(5),

    BufferTooSmall(6);


    private final int value;

    SimError(int value) {
        this.value = value;
    }

    @Override
    public int intValue() {
        return value;
    }
}


