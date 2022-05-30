# nwlocker
Near Watch Locker 

site: <a href="https://near.ws" target="_blank">near.ws</a>

### Build
```
$ cargo build --target wasm32-unknown-unknown --release
$ copy target\wasm32-unknown-unknown\release\nwlocker.wasm contract.wasm 
```
### Deploy
```
$ near deploy --wasmFile contract.wasm --accountId account.near
```

### Initialization
```
$ near call account.near init '{\"pairs\":[[\"\",\"\"]]}' --accountId account.near --networkId mainnet --nodeUrl https://rpc.mainnet.near.org
```

### Support
<a href="https://t.me/nearwatch" target="_blank">Near.Watch technical support group (telegram)</a>
