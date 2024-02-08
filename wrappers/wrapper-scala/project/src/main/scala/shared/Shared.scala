package shared

import java.nio.file.{Files, Path}

object Shared {

  val CLibName = "libnetsim.so"

  val TargetForSharedObjectDownload = "../../target/release/"

  def tempPathForSharedObject: Path = Files.createTempFile("so_download", "gzip")

  def targetPathForSharedObjectDownload: Path =
    Files.createDirectories(
      Path.of(TargetForSharedObjectDownload)
    )

  val pathToBlsHeaderOrigin = Path.of("../../")

  def cLibLocation: String = targetPathForSharedObjectDownload.resolve(CLibName).toString

  private def toStandardString(s: String): String = s.toLowerCase.replace("\\s+", "_")

  def pathToNativeObjectsInJar: Path =
    Path.of("NATIVE",
      toStandardString(sys.props("os.arch")),
      toStandardString(sys.props("os.name")))


}
