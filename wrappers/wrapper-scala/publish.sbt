
organization := "ce.iohk"

javacOptions ++= Seq("-source", "11", "-target", "11")

ThisBuild / licenses := List("APL2" -> url("https://www.apache.org/licenses/LICENSE-2.0.txt"))
ThisBuild / description := "A scala interface to the netsim library"

ThisBuild / scmInfo := Some(
  ScmInfo(
    url("https://github.com/input-output-hk/ce-netsim"),
    "scm:git@github.com/input-output-hk/ce-netsim.git"
  )
)

ThisBuild / developers := List(
  Developer("mcsherrylabs", "Alan McSherry", "alan.mcsherry@iohk.io", url("https://github.com/mcsherrylabs"))
)

// Remove all additional repository other than Maven Central from POM
ThisBuild / pomIncludeRepository := { _ => false }

publishMavenStyle := true

ThisBuild / versionScheme := Some("early-semver")

resolvers += "stepsoft" at "https://nexus.mcsherrylabs.com/repository/releases"

resolvers += "stepsoft-snapshots" at "https://nexus.mcsherrylabs.com/repository/snapshots"

updateOptions := updateOptions.value.withGigahorse(false)

ThisBuild / publishTo := {
  val nexus = "https://nexus.mcsherrylabs.com/"
  if (isSnapshot.value)
    Some("snapshots" at nexus + "repository/snapshots")
  else
    Some("releases"  at nexus + "repository/releases")
}

ThisBuild / credentials += sys.env.get("NEXUS_USER").map(userName => Credentials(
  "Sonatype Nexus Repository Manager",
  "nexus.mcsherrylabs.com",
  userName,
  sys.env.getOrElse("NEXUS_PASS", ""))
).getOrElse(
  Credentials(Path.userHome / ".ivy2" / ".credentials.mcsherrylabs")
)

val fallbackVerison = "0.0.3"

ThisBuild / version := sys.env.getOrElse("GITHUB_REF_NAME", fallbackVerison).replaceAll("/", "_")

