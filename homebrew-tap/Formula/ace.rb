class Ace < Formula
  desc "Accelerated Coding Environment — setup and manage AI coding assistants"
  homepage "https://ace-rs.dev"
  license "MIT"
  version "0.9.3"

  url "https://github.com/ace-rs/ace/releases/download/v0.9.3/ace-aarch64-apple-darwin"
  sha256 "41ad5406b1faeb66d8b39709fed0db4302037436dd46feabd5076ec8ff87ecd2"

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
