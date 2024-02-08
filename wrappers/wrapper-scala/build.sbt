enablePlugins(BuildInfoPlugin)
enablePlugins(JavaAppPackaging)


import scala.language.postfixOps
import shared.Shared._

name := "ce-netsim"

scalaVersion := "2.13.10"

organization := "ce.iohk"

// Make these values available to the project source at compile time
buildInfoKeys ++= Seq[BuildInfoKey](
  "NameOfBbsSharedObject" -> CLibName,
  "pathToNativeObjectsInJar" -> pathToNativeObjectsInJar,
  "TargetForSharedObjectDownload" -> TargetForSharedObjectDownload
)

resolvers += "jitpack" at "https://jitpack.io"

libraryDependencies ++= Seq(
  "com.github.jnr" % "jnr-ffi" % "2.2.13",
  "org.scalatest" %% "scalatest" % "3.2.15" % Test
)

run / fork := true

// Add the and the  .so to the packaged jar
Compile / packageBin / mappings += {
  (baseDirectory.value / cLibLocation) -> pathToNativeObjectsInJar.resolve(CLibName).toString
}



