## What and why

<!-- One or two sentences: what this changes and the problem it solves. -->

## Checklist

- [ ] The commands in [CONTRIBUTING.md](https://github.com/albertosca/corpus-cartographer/blob/main/CONTRIBUTING.md#run-what-ci-runs) pass locally
- [ ] A change someone could see has an entry under `Unreleased` in `CHANGELOG.md`
- [ ] If the output changed: goldens regenerated with `UPDATE_GOLDEN=1 cargo test` and the diff read
- [ ] If the contract changed: the schema in `contracts/` is updated in the same commit
- [ ] If a README changed: both `README.md` and `README.pt.md`
