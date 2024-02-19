package netsim

import helper.SimError
import jnr.ffi.byref.NativeLongByReference
import jnr.ffi.{NativeLong, Pointer}


class NetSimSocket(val context: NetSimContext,
                   socketPtr: Pointer,
                   val simId: Long)(implicit val netSimNative: NetSimNative) {

  def send(msg: NetSimMsg): SimError = {
    //TODO how is this memory freed?
    val sendArrayPointer = netSimNative.runtime.getMemoryManager.allocateTemporary(msg.msg.length, false)
    sendArrayPointer.put(0, msg.msg, 0, msg.msg.length)
    val simIdNative = new NativeLong(msg.addr)
    netSimNative.netsim_socket_send_to(socketPtr, simIdNative, sendArrayPointer, msg.msg.length)
  }

  def blockingReceive(maxSizeMessage: Int = 1024): Either[SimError, NetSimMsg] = {

    val recvArrayPointer = netSimNative.runtime.getMemoryManager.allocateTemporary(1024, false)
    val recvSize = new NativeLong(maxSizeMessage)
    val recvSizeRef = new NativeLongByReference(recvSize)
    val recvSimId = new NativeLongByReference()

    val result = netSimNative.netsim_socket_recv(
      socketPtr,
      recvArrayPointer,
      recvSizeRef,
      recvSimId
    )
    val len = recvSizeRef.longValue().toInt
    if (result == SimError.Success) {
      //TODO cant handle larger than Int

      val ary = new Array[Byte](len)
      recvArrayPointer.get(0, ary, 0, len)
      //(0 until len) foreach (i => ary(i) = recvArrayPointer.getByte(i))
      val addrFrom = recvSimId.longValue()
      Right(NetSimMsg(addrFrom, ary))
    } else {
      Left(result)
    }
  }

  def release(): SimError = {
    netSimNative.netsim_socket_release(socketPtr)
  }
}
