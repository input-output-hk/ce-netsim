enablePlugins(BuildInfoPlugin)
enablePlugins(JavaAppPackaging)


import sbt.Keys.libraryDependencies

import scala.language.postfixOps
import shared.Shared.*

name := "ce-netsim-ffi"

scalaVersion := "2.13.10"

organization := "ce.iohk"

// Make these values available to the project source at compile time
buildInfoKeys ++= Seq[BuildInfoKey](
  "NameOfSharedObject" -> CLibName,
  "pathToNativeObjectsInJar" -> pathToNativeObjectsInJar,
  "TargetForSharedObjectDownload" -> TargetForSharedObjectDownload
)

resolvers += "jitpack" at "https://jitpack.io"

libraryDependencies ++= Seq(
  "com.github.jnr" % "jnr-ffi" % "2.2.16",
  "com.mcsherrylabs" %% "sss-events" % "0.0.6" % Test,
  "ch.qos.logback" % "logback-classic" % "1.4.4" % Test,
  "org.scalatest" %% "scalatest" % "3.2.15" % Test
)

run / fork := true

// Add the and the  .so to the packaged jar
Compile / packageBin / mappings += {
  (baseDirectory.value / cLibLocation) -> pathToNativeObjectsInJar.resolve(CLibName).toString
}



