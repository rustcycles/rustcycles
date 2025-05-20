# Release checklist

- Bump version
- Update CHANGELOG.md
- Commit, `git push`, make sure CI passes
- `cargo publish`
- `git tag -a vX.Y.Z`
- `git push` the tag
