class Ace < Formula
  desc "Accelerated Coding Environment — setup and manage AI coding assistants"
  homepage "https://ace-rs.dev"
  license "MIT"
  version "0.9.2"

  url "https://github.com/ace-rs/ace/releases/download/v0.9.2/ace-aarch64-apple-darwin"
  sha256 "c942953bbf1cccdb92f639d8078c01d1e01337550689e956bf2efe5a1ac35cd7"

  depends_on arch: :arm64
  depends_on :macos

  def install
    bin.install "ace-aarch64-apple-darwin" => "ace"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/ace version")
  end

  livecheck do
    url "https://ace-rs.dev/latest"
    regex(/v?(\d+(?:\.\d+)+)/i)
  end
end
