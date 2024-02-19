package netsim

import buildinfo.BuildInfo
import helper.SimError
import netsim.NetSimOps.ContextOp
import netsim.Ops.OrThrow
import sss.events.EventProcessor.EventHandler
import sss.events.LogFactory.log
import sss.events.{EventProcessingEngine, EventProcessor, EventProcessorSupport}

import scala.concurrent.duration.DurationInt

object Main  {

  def main(args: Array[String]): Unit = {
    val netsim = NetSimNative(
      Seq(BuildInfo.TargetForSharedObjectDownload, ".")
    )
    implicit val engine = EventProcessingEngine()
    //thread num must be one greater than number of blocking receives
    engine.start(4)

    val ep = new EventProcessorSupport {

      val context = netsim.newContext().orThrow
      val recvSocket = context.openSocket().orThrow
      val sendSocket = context.openSocket().orThrow

      override def createOnEvent(self: EventProcessor): EventHandler = {
        val listener = new BlockingRecv(self, 1024)
        var count = 0

        {

          case "LISTEN" =>
            listener ! recvSocket
            self.engine.scheduler.schedule(self.id, "STOP", 1.minutes)

          case fromRevc: NetSimMsg =>
            val s = new String(fromRevc.msg)
            log.info(s"Got a msg from ${fromRevc.addr} $s")


          case "SEND" =>
            println("Begin send")
            val msg = "HELLO " + count
            count += 1
            println(s"msg: $msg")
            val m = NetSimMsg(recvSocket.simId, msg.getBytes)
            sendSocket.send(m) match {
              case SimError.Success =>
                //self.engine.scheduler.schedule(self.id, "SEND", 1.millis)
                self ! "SEND"
              case problem =>
                println(s"PROBLEM $problem")
            }


          case "STOP" =>
            self.become {
              case "SHUTDOWN" =>
                println(s"Recv Socket release ${recvSocket.release()}")
                println(s"Recv Socket release ${sendSocket.release()}")
                val result = context.shutdown()
                println(s"Shutdown $result")
            }
            self ! "SHUTDOWN"
        }

      }
    }
    val eventProcessor = engine.newEventProcessor(ep)

    eventProcessor ! "LISTEN"
    eventProcessor ! "SEND"

  }

}

