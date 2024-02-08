import java.util.Base64

package object netsim {

  object Ops {
    implicit class ByteAryOps(val ary: Array[Byte]) extends AnyVal {
      def toBase64Str: String = Base64.getEncoder.encodeToString(ary)
    }

    implicit class StrOps(val s: String) extends AnyVal {
      def toByteAry: Array[Byte] = Base64.getDecoder.decode(s)
    }
  }
}
