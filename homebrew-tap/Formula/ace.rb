class Ace < Formula
  desc "Accelerated Coding Environment — setup and manage AI coding assistants"
  homepage "https://ace-rs.dev"
  license "MIT"
  version "0.8.1"

  url "https://github.com/ace-rs/ace/releases/download/v0.8.1/ace-aarch64-apple-darwin"
  sha256 "011e3e7d6fb87ddedf66a61b3e476b885e8a611ae1d4f865bdfb5e62928f8489"

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
