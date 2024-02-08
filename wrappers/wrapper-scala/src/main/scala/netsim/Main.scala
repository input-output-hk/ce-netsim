package netsim

import netsim.Ops.ByteAryOps
import buildinfo.BuildInfo
import helper.ArrayStruct
import helper.ArrayStruct.Ops.{ByteAryFromPointerRef, ByteAryToPointer}
import jnr.ffi.byref.NativeLongByReference

import java.util.Base64
import scala.util.Random

object Main  {

  def main(args: Array[String]): Unit = {
    val netsim = NetSimNative(
      Seq(BuildInfo.TargetForSharedObjectDownload, ".")
    )
    import netsim.runtime
    val buf = netsim.makeInStruct()

    val nativeLong = new NativeLongByReference()
    var isOk = netsim.receive_ffi(buf, nativeLong)
    println(s"isOk? ${isOk}")
    println(s"addr ${nativeLong.getValue.longValue()}")
    val asStr = buf.toByteAry.toBase64Str
    println(s"buf ${asStr}")
    //println(s"addr ${nativeLong.getValue.longValue()}")

    val sendBuf = "HELLO".getBytes.toStructPointer

    isOk = netsim.send_ffi(9, sendBuf)
    println(s"Send isOk? ${isOk}")

    println("Done")
  }

}
