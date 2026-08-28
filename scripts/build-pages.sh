#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
site_dir="${1:-$repo_root/site}"
if [[ "$site_dir" != /* ]]; then
  site_dir="$(pwd)/$site_dir"
fi
worktree_root="$(mktemp -d)"

cleanup() {
  git -C "$repo_root" worktree remove --force "$worktree_root/release" >/dev/null 2>&1 || true
  rm -rf "$worktree_root"
}
trap cleanup EXIT

rm -rf "$site_dir"
mkdir -p "$site_dir"

mkdir -p "$site_dir/cli"

MDBOOK_OUTPUT__HTML__SITE_URL="/cli/dev/" \
  mdbook build "$repo_root/docs" --dest-dir "$site_dir/cli/dev"

latest_track=""
while IFS= read -r track; do
  [[ -n "$track" ]] || continue
  tag="$({
    git -C "$repo_root" tag --list "derivon-cli-v${track}.*" --sort=-version:refname
  } | grep -E "^derivon-cli-v${track}\.[0-9]+$" | sed -n '1p')"
  [[ -n "$tag" ]] || continue
  if ! git -C "$repo_root" cat-file -e "$tag:docs/book.toml" 2>/dev/null; then
    continue
  fi

  git -C "$repo_root" worktree add --quiet --detach "$worktree_root/release" "$tag"
  MDBOOK_OUTPUT__HTML__SITE_URL="/cli/v${track}/" \
    mdbook build "$worktree_root/release/docs" --dest-dir "$site_dir/cli/v${track}"
  git -C "$repo_root" worktree remove --force "$worktree_root/release"
  latest_track="$track"
done < <(
  git -C "$repo_root" tag --list 'derivon-cli-v*' \
    | sed -nE \
      -e 's/^derivon-cli-v(0\.[0-9]+)\.[0-9]+$/\1/p' \
      -e 's/^derivon-cli-v([1-9][0-9]*)\.[0-9]+\.[0-9]+$/\1/p' \
    | sort -Vu
)

if [[ -n "$latest_track" ]]; then
  destination="v${latest_track}/"
else
  destination="dev/"
fi

cat > "$site_dir/cli/index.html" <<EOF
<!doctype html>
<meta charset="utf-8">
<meta http-equiv="refresh" content="0; url=${destination}">
<link rel="canonical" href="${destination}">
<title>Derivon CLI documentation</title>
<p><a href="${destination}">Open the Derivon CLI documentation</a></p>
EOF

cat > "$site_dir/index.html" <<'EOF'
<!doctype html>
<meta charset="utf-8">
<meta http-equiv="refresh" content="0; url=cli/">
<link rel="canonical" href="cli/">
<title>Derivon documentation</title>
<p><a href="cli/">Open the Derivon CLI documentation</a></p>
EOF

cp "$repo_root/scripts/install.sh" "$site_dir/cli/install.sh"
touch "$site_dir/.nojekyll"
