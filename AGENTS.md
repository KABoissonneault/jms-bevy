# JMS Project Practices

- Feature requests are usually accompanied by a tech design document
- Dev-only features (such as debug or tooling) do not need a tech design document
- Documents under `**/doc/vault` use a wiki format, and may contain [[references]] to other documents in the vault
- Tech design documents are found under `**/doc/vault/jms/Tech Design` as *.md files
- Changes should usually only be to code and code configuration files, and agent configuration. Changes to game assets or docs are not welcome unless strictly necessary.
- No need to do git operations such as checkout, commit, or push. This is handled manually.
- Features under the dev config do not need tests. Tests will only be run on the normal config.
