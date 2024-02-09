package netsim

import netsim.NetSimBridge.NetSimSend
import sss.events.events.EventProcessor.EventHandler
import sss.events.events.{EventProcessingEngine, EventProcessor}

class TestReplica(implicit engine: EventProcessingEngine)  extends EventProcessor {

  private val initialMessage = "HELLOTHERE "
  private var count = 0
  override def id = "99"

  post(initialMessage.getBytes())

  override protected val onEvent: EventHandler = {

    case data: Array[Byte] =>
      val got = new String(data)
      logInfo(s"$got")
      val msg = initialMessage.substring(0, initialMessage.length) + count.toString
      count += 1
      engine.registrar.post("NetSimBridge", NetSimSend(id.toLong, msg.getBytes()))
  }

}
