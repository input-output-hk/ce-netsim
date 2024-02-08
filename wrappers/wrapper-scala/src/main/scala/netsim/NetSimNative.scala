package netsim

import helper.ArrayStruct
import jnr.ffi.annotations.{In, Out}
import jnr.ffi.byref.NativeLongByReference
import jnr.ffi.{LibraryLoader, Pointer, Runtime}
import jnr.ffi.types._


object NetSimNative {

  type EExternError[T] = Either[ErrorCodeMsg, T]
  type FfiStr = String
  type Handle = Long@u_int64_t
  case class ErrorCodeMsg(code: Long, message: String) extends Exception(s"code: $code, $message")

  implicit class NativeOps(val api: NetSimNative) extends AnyVal {
    def makeInStruct(): Pointer = ArrayStruct.byteArrayStructIn(api.runtime)
  }

  def apply(): NetSimNative = apply(
    Seq(ClasspathSharedObject.createTempFolderWithExtractedLibs.toString)
  )

  def apply(pathsToSearch: Seq[String],
            libsToLoad: Seq[String] = ClasspathSharedObject.namesOfSharedObjectsToLoad): NetSimNative = {

    val withPathsToSearch = pathsToSearch.foldLeft(LibraryLoader.create(classOf[NetSimNative])) {
      case (acc, e) => acc.search(e)
    }
    val withLibsToLoadAndPathsToSearch = libsToLoad.foldLeft(withPathsToSearch) {
      case (acc, e) => acc.library(e)
    }

    withLibsToLoadAndPathsToSearch.load()

  }


  def externSuccess[T](t: T): EExternError[T] = Right(t)

  def eitherExternErrorOr(externError: ExternError, returnCode: Long): EExternError[Long] = {
    eitherExternErrorOr(externError, returnCode, returnCode)
  }

  def eitherExternErrorOr[T](externError: ExternError, returnCode: Long, t : => T): EExternError[T] = {
    if(externError.code.get() != 0) {
      Left(ErrorCodeMsg(externError.code.get(), externError.message.get()))
    } else if (returnCode != 0) {
      Left(ErrorCodeMsg(returnCode, s"Return code indicates failure $returnCode"))
    } else {
      Right(t)
    }
  }

}

trait NetSimNative {

  implicit def runtime: Runtime = Runtime.getRuntime(NetSimNative.this)

  //Trivial function serves as a baseline, can be removed
  def add_numbers(a: Int @int32_t, b: Int @int32_t): Long @int32_t

  def receive_ffi(data: Pointer, addr: NativeLongByReference @Out @u_int64_t): Boolean
  def send_ffi(addr: Long @In @u_int64_t, data: Pointer): Boolean


}
