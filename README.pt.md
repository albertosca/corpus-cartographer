🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

[![CI](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)
[![Lint](https://img.shields.io/github/actions/workflow/status/albertosca/corpus-cartographer/ci.yml?branch=main&job=lint&label=lint)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)

# corpus-cartographer

Um motor de linha de comando que desenha um corpus como um mapa. Lê um documento JSON `engine-export` (temas, itens e os termos de cada item) e, opcionalmente, um diretório de embeddings de termos, e escreve as ligações entre itens mais dois map specs que o [`corpus-map`](https://github.com/albertosca/corpus-map) sabe desenhar.

## Instalar e rodar

```sh
cargo install --path .
corpus-cartographer --input examples/hiking-trails.json --out-dir /tmp/trails-map
```

Sem `--embeddings-dir`, cada termo é um eixo próprio, então dois itens se parecem só quando compartilham termos. Com um diretório que tenha `vocab.json` e `embeddings.safetensors`, termos parecidos também contam como parecidos. Ajuste das ligações: `--floor` e `--max-links`.

## Saída

| Arquivo | O que é |
|---|---|
| `links.json` | `corpus-cartographer/engine-links`: ligações entre itens, posições, avisos, contagens |
| `map-spec.v1.json` | territórios e nós para o `corpus-map` |
| `map-spec.nested.json` | o mesmo com territórios aninhados e arestas |

## Contrato v3

A entrada é `corpus-cartographer/engine-export`, versão 3. O schema é `contracts/engine-export.v3.schema.json` e um exemplo completo é `contracts/engine-export.v3.json`.

| Campo | Significado |
|---|---|
| `themes[]` | `id`, `parent_id` (null para tema de topo), `name`, `terms` |
| `items[]` | `id`, `title`, `subtheme_id`, `status` (`assigned`, `needs_owner`, `unclassified`), `badges`, `terms` |
| `items[].badges` | textos que quem produz quer ver no nó do item no mapa; o motor só copia e nunca pesa |
| `items[].terms[]` | `label` (como quem produz chama) e `term` (a palavra comparada; null quando desconhecida) |
| `previous` | posições da última execução, para o mapa manter a forma |
| `rejected_pairs` | pares de itens que nunca devem ser ligados |

A saída segue a versão da entrada. O export v2 (os nomes da época das receitas) ainda é aceito nesta versão e sai na 0.5.0.

## Funciona com

Combina com o motor Rails `corpus_atlas`, que produz o export e importa as ligações, e com o visualizador `corpus-map`, que desenha o mapa. Cada um também funciona sozinho.

## Licença

MIT.
