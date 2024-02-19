package netsim

import sss.events.EventProcessor.EventHandler
import sss.events.{BaseEventProcessor, EventProcessingEngine, EventProcessor}

class BlockingRecv(override val parent: EventProcessor,
                   maxSizeMessage: Int = 1024)(implicit override val engine: EventProcessingEngine) extends BaseEventProcessor {

  override protected val onEvent: EventHandler = {
    case socket: NetSimSocket =>
      become(withPtr(socket))
      self ! "LISTEN"
  }

  private def withPtr(socket: NetSimSocket): EventHandler = {
    case "LISTEN" =>

      socket.blockingReceive(maxSizeMessage) match {
        case Left(issue) =>
          parent ! issue
          unbecome()
        case Right(ary) =>
          parent ! ary
          self ! "LISTEN"
      }

  }
}
