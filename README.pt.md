🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

# corpus-cartographer

Motor de linha de comando que desenha um acervo como mapa: lê um documento JSON `engine-export` (temas, itens e os termos de cada item), opcionalmente um diretório de embeddings dos termos, e escreve os links entre itens e dois map specs.

```sh
cargo run -- --input examples/hiking-trails.json --out-dir /tmp/trails
```

Em andamento: o contrato está sendo renomeado para palavras genéricas (v3), e o v2 continua legível.
