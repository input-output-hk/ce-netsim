package helper;
import jnr.ffi.Runtime;
import jnr.ffi.Struct;

public class NodeId extends Struct {

    public final Unsigned64 id = new Unsigned64();


    public NodeId(Runtime runtime) {
        super(runtime);
    }

}


