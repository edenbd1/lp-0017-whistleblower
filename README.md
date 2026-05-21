# LP-0017: Whistleblower

Censorship-resistant document upload and indexing on the Logos stack.

A Logos Basecamp app that uploads a document to Logos Storage, broadcasts the resulting CID over Logos Delivery so it is immediately discoverable, and optionally anchors the CID on-chain via a LEZ registry program. A permissionless batch CLI lets any third party gather broadcasted CIDs and commit them on-chain in a single transaction — with no coordination required from the original publisher.

> Submission in progress for [LP-0017 on ns.com](https://ns.com/earn/lp-0017-whistleblower-censorship-resistant-document-upload-and-indexing-basecamp-app). Brief: [`prizes/LP-0017.md`](https://github.com/logos-co/lambda-prize/blob/main/prizes/LP-0017.md).

## Status

Work in progress. Scaffolding the workspace; design + recon docs first.

## Layout (planned)

```
crates/
  registry-core/   Shared Borsh types for the on-chain CID registry.
  indexing/        Agnostic document-indexing module (StorageClient + DeliveryClient + RegistryClient traits).
  batch-anchor/    Permissionless CLI: subscribe → dedup → batch-anchor.
  ffi/             cdylib bridge for the Basecamp Qt module.
methods/guest/     SPEL #[lez_program] for the registry.
app/whistleblower/ Basecamp Qt6/QML plugin.
docs/              recon, design, ADRs.
infra/             docker-compose for nwaku + storage locally.
scripts/           demo.sh, setup.sh, ci-local.sh.
```

## License

Dual-licensed under [MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE).
