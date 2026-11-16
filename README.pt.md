🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

[![CI](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)
[![Lint](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml)
[![Coverage](https://img.shields.io/badge/coverage-98.78%25-brightgreen)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)

# corpus-cartographer

**Transforma uma pilha de itens com termos num mapa: quais itens se relacionam, por quê, e onde cada um fica.**

Você entrega itens (artigos, chamados de suporte, papers, trilhas de caminhada), cada um com alguns termos e dentro de uma árvore de temas. O motor acha os vizinhos mais próximos de cada item e explica cada ligação pelos termos que os dois compartilham. Ele também posiciona os temas e os itens num mapa 2D. A saída é JSON puro. Dá para usar só as ligações, ou entregar os map specs à biblioteca React [`corpus-map`](https://github.com/albertosca/corpus-map), que os desenha como uma paisagem com zoom ([demo ao vivo](https://albertosca.github.io/corpus-map/)).

É um binário único. A mesma entrada sempre dá a mesma saída, e ele lê as posições da última execução, então o mapa mantém a forma conforme o corpus cresce. O exemplo de 15 itens roda em menos de 10 ms.

```
export.json ──▶ corpus-cartographer ──▶ links.json            links, positions, warnings
(+ embeddings,                       ├─▶ map-spec.v1.json      ──▶ corpus-map
   optional)                         └─▶ map-spec.nested.json  ──▶ corpus-map
```

## Começo rápido

Baixe um binário para macOS (Apple silicon ou Intel) ou Linux x86_64 na [última release](https://github.com/albertosca/corpus-cartographer/releases/latest). Para compilar do código-fonte você precisa do Rust 1.88 ou mais novo ([rustup](https://rustup.rs)):

```sh
git clone https://github.com/albertosca/corpus-cartographer
cd corpus-cartographer
cargo install --path .
corpus-cartographer --input examples/hiking-trails.json --out-dir trails-map
```

Ele imprime um resumo de uma linha e escreve três arquivos em `trails-map/`:

```
items=15 placed=15 waiting=0 with_vector=14 links=6 warnings=0
```

Cada ligação em `trails-map/links.json` diz o quanto dois itens se parecem (similaridade de cosseno, de -1 a 1) e quais termos eles compartilham:

```json
{
  "source": 101,
  "target": 102,
  "similarity": 0.4894,
  "crosses_theme": false,
  "shared": [
    { "term": "ridge", "source_labels": ["ridge"], "target_labels": ["ridge"] },
    { "term": "summit", "source_labels": ["summit"], "target_labels": ["summit"] },
    { "term": "wind", "source_labels": ["wind"], "target_labels": ["wind"] }
  ]
}
```

Para ver o mapa, instale o `corpus-map` pelo npm e passe o conteúdo de `trails-map/map-spec.nested.json` ao componente `<CorpusMap spec={...} />`, como mostra o [README do corpus-map](https://github.com/albertosca/corpus-map#usage).

## Seus próprios dados

O motor não lê documentos nem extrai termos, então quem fornece os termos é você. Podem ser palavras-chave, tags, categorias ou entidades extraídas, o que o seu sistema já tiver. Os temas formam uma árvore de temas de topo e seus subtemas, e cada item fica num subtema. Este é um export completo:

```json
{
  "schema": "corpus-cartographer/engine-export",
  "version": 3,
  "taxonomy_version": 1,
  "themes": [
    { "id": 1, "parent_id": null, "name": "Databases", "terms": [] },
    { "id": 11, "parent_id": 1, "name": "Indexing", "terms": [] },
    { "id": 2, "parent_id": null, "name": "Networking", "terms": [] },
    { "id": 21, "parent_id": 2, "name": "Protocols", "terms": [] }
  ],
  "items": [
    { "id": 1, "title": "B-trees from scratch", "subtheme_id": 11, "status": "assigned", "badges": [],
      "terms": [{ "label": "B-tree", "term": "b-tree" }, { "label": "index", "term": "index" }, { "label": "storage", "term": "storage" }] },
    { "id": 2, "title": "Why my query ignores the index", "subtheme_id": 11, "status": "assigned", "badges": [],
      "terms": [{ "label": "index", "term": "index" }, { "label": "query plan", "term": "query plan" }, { "label": "storage", "term": "storage" }] },
    { "id": 3, "title": "TCP slow start", "subtheme_id": 21, "status": "assigned", "badges": [],
      "terms": [{ "label": "TCP", "term": "tcp" }, { "label": "congestion", "term": "congestion" }, { "label": "latency", "term": "latency" }] },
    { "id": 4, "title": "QUIC in one page", "subtheme_id": 21, "status": "assigned", "badges": [],
      "terms": [{ "label": "QUIC", "term": "quic" }, { "label": "congestion", "term": "congestion" }, { "label": "latency", "term": "latency" }] }
  ],
  "previous": null,
  "rejected_pairs": []
}
```

```sh
corpus-cartographer --input articles.json --out-dir articles-map --floor 0.2
```

Isso liga os dois artigos de banco de dados entre si, e os dois de redes entre si. O `--floor 0.2` está ali porque o corpus é minúsculo. Um termo pesa mais quanto menos itens o usam, e o que todo item compartilha não conta nada. Com quatro itens isso mantém as similaridades baixas, e o piso padrão de 0.4 não desenharia nenhuma ligação. As 15 trilhas do começo rápido já se ligam com o padrão.

| Opção | Padrão | O que faz |
|---|---|---|
| `--input` | obrigatória | o export a ler |
| `--out-dir` | obrigatória | onde vão os três arquivos de saída, criado se não existir |
| `--embeddings-dir` | nenhum | faz termos relacionados contarem como parecidos, veja [abaixo](#ligações-melhores-com-embeddings) |
| `--floor` | `0.4` | a menor similaridade de cosseno que ainda vira ligação, entre -1 e 1 |
| `--max-links` | `3` | o máximo de ligações mantidas por item; ligações para outros temas são tentadas primeiro |

## Ligações melhores com embeddings

Isto é opcional. Sem embeddings, dois itens só se parecem quando usam exatamente os mesmos termos. Com embeddings, termos relacionados também contam: "beach" e "shore" têm similaridade de cosseno 0,54 com o `potion-base-8M`.

O motor lê um diretório com dois arquivos. O `vocab.json` liga cada termo a um número de linha. O `embeddings.safetensors` guarda um único tensor f32 chamado `embeddings`, com uma linha por termo. O próprio motor normaliza as linhas. Os termos que faltam no vocabulário são contados num aviso `unknown_terms`.

O [`scripts/build_embeddings.py`](scripts/build_embeddings.py) monta esse diretório a partir do seu export com um modelo estático do [Model2Vec](https://github.com/MinishLab/model2vec), que não precisa de GPU nem de PyTorch:

```sh
pip install model2vec
python scripts/build_embeddings.py examples/hiking-trails.json minishlab/potion-base-8M trails-embeddings
corpus-cartographer --input examples/hiking-trails.json --embeddings-dir trails-embeddings --out-dir trails-map
```

No exemplo das trilhas, o mapa passa de 6 para 14 ligações. As trilhas de montanha passam a se ligar entre si mesmo onde dividem menos termos, e o mesmo acontece com as de costa. Rode o script de novo sempre que o export ganhar termos. Qualquer um destes modelos serve:

| Modelo | Download | Idiomas | Licença |
|---|---|---|---|
| [`minishlab/potion-base-8M`](https://huggingface.co/minishlab/potion-base-8M) | 30 MB | Inglês | MIT |
| [`minishlab/potion-base-32M`](https://huggingface.co/minishlab/potion-base-32M) | 129 MB | Inglês | MIT |
| [`minishlab/potion-multilingual-128M`](https://huggingface.co/minishlab/potion-multilingual-128M) | 512 MB | 101, português incluído | MIT |

Use o modelo multilíngue quando os termos não estiverem em inglês. "Praia" e "litoral" têm similaridade de cosseno 0,14 com o modelo em inglês e 0,64 com o multilíngue.

## Saída

| Arquivo | O que é |
|---|---|
| `links.json` | `corpus-cartographer/engine-links`: ligações entre itens, posições, avisos, contagens |
| `map-spec.v1.json` | territórios e nós para o `corpus-map` |
| `map-spec.nested.json` | o mesmo com territórios aninhados, mais as ligações como arestas |

## Como funciona, e os seus limites

- **Vetores dos itens.** O vetor de cada item é a média ponderada dos vetores dos seus termos, menos a média do corpus. Um termo pesa mais quanto menos itens o usam, então o que todo item compartilha não conta nada. As ligações são o cosseno entre dois vetores de itens.
- **Layout.** O layout roda escalonamento multidimensional clássico sobre 1 − cosseno em cada nível: temas de topo, os subtemas dentro deles e os itens dentro de cada subtema. É uma solução fechada, sem semente aleatória, então a mesma entrada dá os mesmos bytes. A contrapartida é que uma figura 2D distorce algumas distâncias, então o mapa é uma vista do corpus e não uma medida.
- **Estabilidade.** Quando `previous` traz as posições da última execução, cada nível é encaixado nelas por rotação, translação e, quando isso encaixa estritamente melhor, reflexão. Por isso um item novo não gira nem espelha o mapa. Os testes seguram isso num limite: acrescentar um item move os territórios não tocados em menos de 10% da extensão do mapa.
- **Itens em espera.** Um item cujo status não é `assigned` espera numa fileira abaixo do mapa, e o resumo o conta como `waiting`.
- **Tamanho.** Sem embeddings cada termo é um eixo, e o motor recusa mais de 4.000.000 de células (o número de termos vezes o maior entre o número de termos e o número de itens com termos) e pede embeddings. O corpus de 5.000 itens abaixo bate nesse limite sem embeddings. Com embeddings, o tempo cresce mais ou menos com o quadrado do número de itens. Medido num Apple M1 rodando outras coisas, com itens sintéticos em 10 temas de 5 subtemas, 5 termos cada, e embeddings de 256 dimensões:

  | Itens | Tempo | Pico de memória |
  |---|---|---|
  | 1.000 | 0,3 s | 21 MB |
  | 2.000 | 1,2 s | 34 MB |
  | 5.000 | 10 s | 69 MB |

  Ele foi feito para corpora de milhares de itens, a escala que o `corpus-map` desenha, e não para milhões.
- **Avisos.** Os avisos ficam em `links.json` e nunca interrompem uma execução:
  - `unknown_terms`: alguns termos faltam no vocabulário, com a contagem.
  - `no_flavour`: um tema não tem vetor, então é posto num anel em volta do pai.
  - `no_alignment`: um nível não pôde ser encaixado em `previous` mesmo compartilhando territórios com ele.
- **Erros.** Um export malformado, um arquivo que falta ou um diretório de embeddings inválido sai com status 1 e uma linha `error:` que nomeia o arquivo e o problema, e não escreve nada.

## Contrato v3

A entrada é `corpus-cartographer/engine-export`, versão 3. O schema é [`contracts/engine-export.v3.schema.json`](contracts/engine-export.v3.schema.json), e os schemas de saída ficam ao lado dele.

| Campo | Significado |
|---|---|
| `schema`, `version` | `"corpus-cartographer/engine-export"` e `3` |
| `taxonomy_version` | a versão que quem produz dá à própria árvore de temas, ou null; o motor não usa |
| `themes[]` | `id`, `parent_id` (null para tema de topo), `name`, `terms` (textos que descrevem o tema) |
| `items[]` | `id`, `title`, `subtheme_id`, `status` (`assigned`, `needs_owner`, `unclassified`), `badges`, `terms` |
| `items[].badges` | textos curtos para mostrar no nó do item no mapa; o motor só copia e nunca pesa |
| `items[].terms[]` | `label` (como quem produz chama) e `term` (a palavra comparada; null quando não há) |
| `previous` | as posições que o `links.json` deu da última vez, ou null, para o mapa manter a forma |
| `rejected_pairs` | pares de itens, id menor primeiro, que nunca devem ser ligados |

A versão da saída segue a versão da entrada. O export v2 ainda é lido na 0.4 e sai na 0.5.0.

## Funciona com

- O [`corpus-map`](https://github.com/albertosca/corpus-map) desenha os map specs.
- O `corpus_atlas`, um motor Rails, escreve o export a partir do banco de uma aplicação e lê as ligações de volta.

Qualquer programa que escreva o JSON acima funciona igual, e cada peça também funciona sozinha.

## Contribuindo

### Rodar o que o CI roda

O crate suporta Rust 1.88 ou mais novo, e o `rust-toolchain.toml` fixa o toolchain usado no desenvolvimento (o rustup instala no primeiro uso). Estes são os comandos de `.github/workflows/`:

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings      # pedantic lints, set in Cargo.toml
cargo install --locked cargo-deny@0.20.2 && cargo deny --locked check
rustup toolchain install 1.88 --profile minimal && cargo +1.88 test --locked
rustup component add llvm-tools-preview && cargo install --locked cargo-llvm-cov@0.9.1
cargo llvm-cov --locked --summary-only --fail-under-lines 98
```

O badge de coverage é uma imagem fixa, então atualize o número nos dois READMEs quando o `cargo llvm-cov` reportar um novo.

### Onde fica cada coisa

Uma execução passa por estes módulos, nesta ordem:

| Passo | Módulo | O que faz |
|---|---|---|
| 1 | `src/export.rs` | lê e confere o export (`src/legacy_v2.rs` lê o v2) |
| 2 | `src/embeddings.rs` | carrega o diretório de embeddings, ou um eixo por termo |
| 3 | `src/vectors.rs` | monta os vetores dos itens e os pesos dos termos |
| 4 | `src/layout.rs`, `src/geometry.rs` | posiciona temas e itens (MDS, alinhamento a `previous`) |
| 5 | `src/links.rs` | acha e explica as ligações (`--floor`, `--max-links`) |
| 6 | `src/output.rs`, `src/warning.rs` | escreve os três documentos e os avisos |

O `src/main.rs` é a linha de comando, e cada arquivo abre com um comentário dizendo pelo que ele responde.

### Mudar o contrato

Os schemas, os exports de exemplo e as saídas `engine-links` de referência (goldens) ficam em `contracts/`, e o golden do map spec do exemplo das trilhas fica em `tests/golden/`. Uma mudança no que o motor lê ou escreve segue assim:

1. Atualize o schema.
2. Regere os goldens com `UPDATE_GOLDEN=1 cargo test`.
3. Leia o diff, e commite o schema e os goldens juntos.

O parser recusa qualquer campo de entrada que o schema não nomeie, então um campo de entrada novo também exige mudança em `src/export.rs`. Um teste separado mantém o vocabulário da primeira aplicação que usou o motor fora do código, dos documentos v3, dos testes e dos dois READMEs; só o leitor do v2 pode usá-lo.

Uma mudança que alguém possa ver entra em `Unreleased` no [CHANGELOG](CHANGELOG.md), e as mudanças de README vão para os dois idiomas. Para lançar uma versão, ajuste a versão no `Cargo.toml`, mova `Unreleased` para baixo dessa versão e faça push da tag `vX.Y.Z`. O `release.yml` recusa uma tag diferente da versão do crate e então compila os três binários.

### O que os testes garantem

O crate tem cinco dependências de runtime (clap, serde, serde_json, safetensors, sha2), e código `unsafe` é proibido. Os testes conferem quatro garantias em cada formato que o motor lê:

- a mesma entrada dá a mesma saída, byte a byte;
- a identidade de um território não depende da ordem da entrada;
- o limite de estabilidade acima se mantém;
- corpora extremos ainda dão saída válida: um corpus vazio, um único item, todos os itens em espera, e itens sem termos.

Eles também conferem as saídas contra os JSON schemas em `contracts/` e contra os goldens, e conferem que arquivos de embeddings malformados são recusados. O `cargo deny` confere avisos de segurança, licenças e origens.

## Status

Versão 0.4.1, antes da 1.0. O contrato do export ainda pode mudar entre versões minor. Cada mudança é versionada e listada no [CHANGELOG](CHANGELOG.md), e a versão anterior continua sendo lida por um tempo.

## Licença

MIT.
