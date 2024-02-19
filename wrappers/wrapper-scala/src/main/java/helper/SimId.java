package helper;
import jnr.ffi.Runtime;
import jnr.ffi.Struct;

public class SimId extends Struct {

    public final Unsigned64 id = new Unsigned64();


    public SimId(Runtime runtime) {
        super(runtime);
    }

}


