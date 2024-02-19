import helper.SimError
import jnr.ffi.byref.PointerByReference


package object netsim {

  case class NetSimMsg(addr: Long, msg: Array[Byte])

  object Ops {
    implicit class OrThrow[R](val e: Either[SimError, R]) {
      def orThrow: R = {
        e match {
          case Left(bad) => throw new RuntimeException(bad.toString)
          case Right(result) => result
        }
      }
    }

  }

  object NetSimOps {
    implicit class ContextOp(val netsim: NetSimNative) extends AnyVal {
      def newContext(): Either[SimError, NetSimContext] = {

        val contextPtr = new PointerByReference()
        val result = netsim.netsim_context_new(contextPtr)
        if(result == SimError.Success) {
          Right(new NetSimContext(contextPtr.getValue)(netsim))
        } else {
          Left(result)
        }

      }
    }
  }
}
