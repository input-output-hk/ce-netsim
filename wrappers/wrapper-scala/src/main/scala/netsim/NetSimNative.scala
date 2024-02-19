package netsim

import helper.SimError
import jnr.ffi.byref.{NativeLongByReference, PointerByReference}
import jnr.ffi.types._
import jnr.ffi.{LibraryLoader, NativeLong, Pointer, Runtime}


object NetSimNative {


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

  def netsim_context_new(context: PointerByReference): SimError
  def netsim_context_shutdown(context: Pointer): SimError
  def netsim_context_open(
                           context: Pointer,
                           output: PointerByReference
                         ): SimError


  def netsim_socket_send_to(
                             socket: Pointer,
                             // where we will put the sender ID
                             to: NativeLong,
                             // pre-allocated byte array
                             msg: Pointer,
                             // the maximum size of the pre-allocated byte array
                             size: Long@u_int64_t,
                           ): SimError

  def netsim_socket_recv(
                          socket: Pointer,
                          // pre-allocated byte array
                          msg: Pointer,
                          // the maximum size of the pre-allocated byte array
                          size: NativeLongByReference,
                          // where we will put the sender ID
                          from: NativeLongByReference,
                        ): SimError

  def netsim_socket_id(socket: Pointer, simId: NativeLongByReference): SimError

  def netsim_socket_release(socket: Pointer): SimError
}
