import mill._, scalalib._

object app extends ScalaModule {
  def scalaVersion = "3.3.1"
  def ivyDeps = Agg(ivy"com.typesafe.akka::akka-http:10.5.0")
}
