🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

[![CI](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)
[![Lint](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml)
[![Coverage](https://img.shields.io/badge/coverage-98.78%25-brightgreen)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)

# corpus-cartographer

**Transforma uma pilha de itens com termos num mapa: quais itens se relacionam, por quê, e onde cada um fica.**

## Instalar

Baixe um binário para macOS (Apple silicon ou Intel) ou Linux x86_64 na [última release](https://github.com/albertosca/corpus-cartographer/releases/latest). Para compilar do código-fonte você precisa do Rust 1.88 ou mais novo ([rustup](https://rustup.rs)):

```sh
git clone https://github.com/albertosca/corpus-cartographer
cd corpus-cartographer
cargo install --path .
```

![O exemplo das trilhas desenhado pelo corpus-map: três territórios, depois um zoom em Mountains até as trilhas](docs/trails-map.gif)

A [demo ao vivo do `corpus-map`](https://albertosca.github.io/corpus-map/) mostra o tipo de mapa que a saída desenha.

## Começo rápido

```sh
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

```
export.json ──▶ corpus-cartographer ──▶ links.json            links, positions, warnings
(+ embeddings,                       ├─▶ map-spec.v1.json      ──▶ corpus-map
   optional)                         └─▶ map-spec.nested.json  ──▶ corpus-map
```

## O que você ganha

- **Ligações explicadas.** Os vizinhos mais próximos de cada item, cada ligação com os termos que os dois compartilham.
- **Um mapa.** Temas, subtemas e itens posicionados em 2D, com os itens que ainda esperam um tema numa fileira abaixo.
- **Um mapa que mantém a forma.** Passe de volta as posições da última execução, e um item novo não gira nem espelha o mapa.
- **Mesma entrada, mesmos bytes.** Um binário único, sem semente aleatória e com cinco dependências de runtime; o exemplo de 15 itens roda em menos de 10 ms e 5.000 itens com embeddings levam uns 10 s.
- **Os seus próprios termos.** Palavras-chave, tags ou entidades que você já tem, opcionalmente com embeddings para termos relacionados contarem como parecidos.
- **JSON puro.** Um contrato versionado com JSON schemas, então qualquer programa escreve a entrada e lê a saída.

## Documentação

A documentação detalhada está em inglês:

- [Uso](docs/usage.md): um export mínimo completo, as opções, embeddings, a saída e o contrato.
- [Como funciona](docs/how-it-works.md): por que existe, o design e as suas contrapartidas, tamanhos medidos, o que os testes garantem.
- [Contribuindo](CONTRIBUTING.md): os comandos do CI, onde fica cada coisa, mudar o contrato, releases.

## Funciona com

- O [`corpus-map`](https://github.com/albertosca/corpus-map) desenha os map specs.
- O `corpus_atlas`, um motor Rails, escreve o export a partir do banco de uma aplicação e lê as ligações de volta.

Qualquer programa que escreva o JSON funciona igual, e cada peça também funciona sozinha.

## Status

Versão 0.4.1, antes da 1.0. O contrato do export ainda pode mudar entre versões minor, cada mudança é listada no [CHANGELOG](CHANGELOG.md), e a versão anterior continua sendo lida por um tempo.

## Licença

MIT.
