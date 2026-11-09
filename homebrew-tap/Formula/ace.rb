class Ace < Formula
  desc "Accelerated Coding Environment — setup and manage AI coding assistants"
  homepage "https://ace-rs.dev"
  license "MIT"
  version "0.10.1"

  url "https://github.com/ace-rs/ace/releases/download/v0.10.1/ace-aarch64-apple-darwin"
  sha256 "5dd3e43191fa306c72a516172880de17042d2b5c5ce7d68422554a1c4949530c"

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
