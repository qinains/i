# Homebrew Tap Formula for i
#
# Repository: https://github.com/qinains/homebrew-i
#
# Usage:
#   brew tap qinains/i
#   brew install i

class I < Formula
  desc "Plugin-first coding agent with Rust core"
  homepage "https://github.com/qinains/i"
  url "https://github.com/qinains/i/releases/download/v0.1.0/i-0.1.0-aarch64-apple-darwin-min-size.tar.gz"
  version "0.1.0"
  sha256 "YOUR_SHA256_HERE"
  license "MIT"
  head "https://github.com/qinains/i.git", branch: "main"

  depends_on "rust" => :build

  def install
    bin.install "bin/i"
  end

  test do
    system "#{bin}/i", "--version"
  end
end
