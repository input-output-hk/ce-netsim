package netsim

import helper.ArrayStruct
import jnr.ffi.annotations.{In, Out}
import jnr.ffi.byref.NativeLongByReference
import jnr.ffi.types._
import jnr.ffi.{LibraryLoader, Pointer, Runtime}


object NetSimNative {

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


}

trait NetSimNative {

  implicit def runtime: Runtime = Runtime.getRuntime(NetSimNative.this)

  def receive_ffi(data: Pointer, addr: NativeLongByReference@Out @u_int64_t): Boolean

  def send_ffi(addr: Long@In @u_int64_t, data: Pointer): Boolean


}
