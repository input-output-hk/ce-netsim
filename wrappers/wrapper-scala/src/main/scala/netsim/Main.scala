package netsim

import buildinfo.BuildInfo
import helper.ArrayStruct.Ops.{ByteAryFromPointerRef, ByteAryToPointer}
import jnr.ffi.byref.NativeLongByReference
import netsim.Ops.ByteAryOps
import sss.events.events.EventProcessingEngine

object Main  {

  def main(args: Array[String]): Unit = {
    val netsim = NetSimNative(
      Seq(BuildInfo.TargetForSharedObjectDownload, ".")
    )
    implicit val engine = EventProcessingEngine()

    val bridge  = new NetSimBridge(netsim)
    val testOne = new TestReplica()

    engine.start(1)

    println("Started...")
  }

}
