package netsim

import buildinfo.BuildInfo
import helper.ArrayStruct.Ops.{ByteAryFromPointerRef, ByteAryToPointer}
import jnr.ffi.byref.NativeLongByReference
import netsim.Ops.ByteAryOps

object Main  {

  def main(args: Array[String]): Unit = {
    val netsim = NetSimNative(
      Seq(BuildInfo.TargetForSharedObjectDownload, ".")
    )
    import netsim.runtime

    var count = 0

    while(count < 100000) {
      count += 1
      val sendBuf = s"HELLO_${count}".getBytes.toStructPointer
      var isOk = netsim.send_ffi(count, sendBuf)
      println(s"Send isOk? ${isOk}")
      val buf = netsim.makeInStruct()
      val nativeLong = new NativeLongByReference()
      isOk = netsim.receive_ffi(buf, nativeLong)
      if(isOk) {
        println(s"addr ${nativeLong.getValue.longValue()}")
        val asStr = new String(buf.toByteAry)
        println(s"buf ${asStr}")
      }
    }

    println("Done")
  }

}
