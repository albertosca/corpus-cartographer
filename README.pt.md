🇺🇸 [English](README.md) · 🇧🇷 [Português](README.pt.md)

[![CI](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)
[![Lint](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml/badge.svg)](https://github.com/albertosca/corpus-cartographer/actions/workflows/lint.yml)
[![Coverage](https://img.shields.io/badge/coverage-98.78%25-brightgreen)](https://github.com/albertosca/corpus-cartographer/actions/workflows/ci.yml)

# corpus-cartographer

Um motor de linha de comando que desenha um corpus como um mapa. Lê um documento JSON `engine-export` (temas, itens e os termos de cada item) e, opcionalmente, um diretório de embeddings de termos, e escreve as ligações entre itens mais dois map specs que o [`corpus-map`](https://github.com/albertosca/corpus-map) sabe desenhar.

## Instalar e rodar

```sh
cargo install --path .
corpus-cartographer --input examples/hiking-trails.json --out-dir /tmp/trails-map
```

Sem `--embeddings-dir`, cada termo é um eixo próprio, então dois itens se parecem só quando compartilham termos. Com um diretório que tenha `vocab.json` e `embeddings.safetensors`, termos parecidos também contam como parecidos ([como montar um](#embeddings)). Ajuste das ligações: `--floor` e `--max-links`.

## Embeddings

O motor lê qualquer diretório com dois arquivos. O `vocab.json` liga cada termo a um número de linha. O `embeddings.safetensors` guarda um único tensor f32 chamado `embeddings`, com uma linha por termo. As linhas não precisam vir normalizadas, porque o motor divide cada uma pela própria norma. Os termos que faltam no vocabulário são contados num aviso `unknown_terms`.

O jeito mais rápido de montar esse diretório é codificar os seus próprios termos com um modelo estático do [Model2Vec](https://github.com/MinishLab/model2vec), que não precisa de GPU nem de PyTorch. Salve isto como `build_embeddings.py`:

```python
import json
import sys
from pathlib import Path

from model2vec import StaticModel
from safetensors.numpy import save_file

export_path, model_name, output_directory = sys.argv[1], sys.argv[2], Path(sys.argv[3])
export = json.loads(Path(export_path).read_text())
terms = sorted(
    {term for theme in export["themes"] for term in theme["terms"]}
    | {entry["term"] for item in export["items"] for entry in item["terms"]}
)
vectors = StaticModel.from_pretrained(model_name).encode(terms).astype("float32")
output_directory.mkdir(parents=True, exist_ok=True)
(output_directory / "vocab.json").write_text(json.dumps({term: index for index, term in enumerate(terms)}))
save_file({"embeddings": vectors}, output_directory / "embeddings.safetensors")
print(f"{len(terms)} terms, {vectors.shape[1]} dimensions")
```

```sh
pip install model2vec
python build_embeddings.py examples/hiking-trails.json minishlab/potion-base-8M /tmp/trails-embeddings
corpus-cartographer --input examples/hiking-trails.json --embeddings-dir /tmp/trails-embeddings --out-dir /tmp/trails-map
```

No exemplo das trilhas, o mapa passa de 6 para 14 ligações. As trilhas de montanha se ligam entre si mesmo quando dividem poucos termos, e o mesmo acontece com as de costa. Rode o script de novo sempre que o export ganhar termos. Qualquer um destes modelos serve no lugar:

| Modelo | Download | Idiomas | Licença |
|---|---|---|---|
| [`minishlab/potion-base-8M`](https://huggingface.co/minishlab/potion-base-8M) | 30 MB | Inglês | MIT |
| [`minishlab/potion-base-32M`](https://huggingface.co/minishlab/potion-base-32M) | 129 MB | Inglês | MIT |
| [`minishlab/potion-multilingual-128M`](https://huggingface.co/minishlab/potion-multilingual-128M) | 512 MB | 101, português incluído | MIT |

Use o modelo multilíngue quando os termos não estiverem em inglês. "Praia" e "litoral" dão 0,14 de similaridade no modelo em inglês e 0,64 no multilíngue.

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

## Desenvolvimento

Rust 1.88 ou mais novo. O CI roda `cargo test`, `cargo fmt --check`, `cargo clippy` com o grupo pedantic, `cargo deny check` (avisos de segurança, licenças, origens), a suíte no Rust 1.88 e `cargo llvm-cov` com piso de 98% das linhas. O badge de coverage mostra o número medido e é atualizado à mão quando ele muda.

## Licença

MIT.
