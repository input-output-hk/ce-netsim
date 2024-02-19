package netsim

import helper.SimError
import jnr.ffi.Pointer
import jnr.ffi.byref.{NativeLongByReference, PointerByReference}



class NetSimContext(contextPtr: Pointer)(implicit netSimNative: NetSimNative) {

  def shutdown(): SimError = {
    netSimNative.netsim_context_shutdown(contextPtr)
  }

  def openSocket(): Either[SimError, NetSimSocket] = {
    val socketPtr = new PointerByReference()

    val resultOpen = netSimNative.netsim_context_open(contextPtr, socketPtr)


    if (resultOpen == SimError.Success) {
      val simId = new NativeLongByReference()
      val resultId = netSimNative.netsim_socket_id(socketPtr.getValue, simId)

      if(resultId == SimError.Success) {
        Right(new NetSimSocket(this, socketPtr.getValue, simId.longValue()))
      } else {
        Left(resultId)
      }

    } else {
      Left(resultOpen)
    }
  }

}
