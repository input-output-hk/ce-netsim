package netsim


import helper.ArrayStruct.Ops.{ByteAryFromPointerRef, ByteAryToPointer}
import jnr.ffi.byref.NativeLongByReference
import netsim.NetSimBridge.NetSimSend
import sss.events.events.EventProcessor.EventHandler
import sss.events.events.{EventProcessingEngine, EventProcessor}
import concurrent.duration._
object NetSimBridge {
  case class NetSimSend(addr: Long, data: Array[Byte])
}

class NetSimBridge(netSimNative: NetSimNative)(implicit eventProcessingEngine: EventProcessingEngine) extends EventProcessor {

  override def id = "NetSimBridge"

  post((netSimNative, 0))

  import netSimNative.runtime

  override protected val onEvent: EventHandler = {

    case (netSimNative: NetSimNative, missCount: Int) =>
      val buf = netSimNative.makeInStruct()
      val nativeLong = new NativeLongByReference()
      val isOk = netSimNative.receive_ffi(buf, nativeLong)
      if(isOk) {
        val addr = nativeLong.getValue.longValue()
        val data = buf.toByteAry
        val addressAsString = addr.toString //might need to fancy this up.
        eventProcessingEngine.registrar.post(addressAsString, data)
        post((netSimNative, 0))
      } else {
        //back off, but max out at 40ms delay
        val newMissCount = missCount + 1
        val delay = Math.min(newMissCount, 40)
        eventProcessingEngine.scheduler.schedule(id, (netSimNative, newMissCount), delay.millis)
      }

    case NetSimSend(addr, data) =>
      val as_bytes = data.toStructPointer
      val isOk = netSimNative.send_ffi(addr, as_bytes)
      if(!isOk) {
        logWarn(s"Failed to send! ")
      }

  }
}
