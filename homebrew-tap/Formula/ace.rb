class Ace < Formula
  desc "Accelerated Coding Environment — setup and manage AI coding assistants"
  homepage "https://ace-rs.dev"
  license "MIT"
  version "0.9.1"

  url "https://github.com/ace-rs/ace/releases/download/v0.9.1/ace-aarch64-apple-darwin"
  sha256 "dd18efdf30b45487ec9808d454e2087706a3b1a008e4c0aa1c87b30a924c1ad2"

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
