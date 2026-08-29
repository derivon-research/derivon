#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "usage: $0 VERSION SHA256 OUTPUT" >&2
  exit 64
fi

version=$1
sha256=$2
output=$3

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || {
  echo "invalid release version: $version" >&2
  exit 64
}
[[ "$sha256" =~ ^[0-9a-f]{64}$ ]] || {
  echo "invalid SHA-256: $sha256" >&2
  exit 64
}

mkdir -p "$(dirname "$output")"
cat > "$output" <<EOF
class Derivon < Formula
  desc "Stateless CLI for weighted directed B-hypergraphs"
  homepage "https://docs.derivon.net/cli/"
  url "https://static.crates.io/crates/derivon-cli/derivon-cli-${version}.crate"
  sha256 "${sha256}"
  license "MIT"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_equal "derivon #{version}\\n", shell_output("#{bin}/derivon -v")
    assert_match "derivon #{version}", shell_output("#{bin}/derivon --version")
    input = "{\"points\":[],\"hyperedges\":[]}\\n"
    expected = "{\"hyperedges\":[],\"points\":[]}\\n"
    assert_equal expected, pipe_output("#{bin}/derivon validate", input)
  end
end
EOF
