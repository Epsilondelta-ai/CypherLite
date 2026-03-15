# CypherLite Translation Guide

This guide explains how to contribute translations to CypherLite documentation.

---

## Directory Structure

All translated README files live in `docs/i18n/`:

```
docs/i18n/
  README.zh.md     — Chinese Simplified (中文)
  README.hi.md     — Hindi (हिन्दी)
  README.es.md     — Spanish (Español)
  README.fr.md     — French (Français)
  README.ar.md     — Arabic (العربية)
  README.bn.md     — Bengali (বাংলা)
  README.pt.md     — Portuguese (Português)
  README.ru.md     — Russian (Русский)
  README.ko.md     — Korean (한국어)
  TRANSLATING.md   — This guide
```

The English source is at the project root: `README.md`.

---

## File Naming Convention

Each translation file uses the ISO 639-1 two-letter language code:

| Language | Code | File |
|----------|------|------|
| Chinese Simplified | zh | README.zh.md |
| Hindi | hi | README.hi.md |
| Spanish | es | README.es.md |
| French | fr | README.fr.md |
| Arabic | ar | README.ar.md |
| Bengali | bn | README.bn.md |
| Portuguese | pt | README.pt.md |
| Russian | ru | README.ru.md |
| Korean | ko | README.ko.md |

To add a new language, create `README.<code>.md` following the same structure.

---

## Required Metadata Header

Every translated file MUST begin with this metadata comment block:

```markdown
<!-- CypherLite Documentation Translation -->
<!-- Source Language: English -->
<!-- Target Language: {Language Name} ({Native Name}) -->
<!-- Last Synced Commit: HEAD -->
<!-- Last Updated: YYYY-MM-DD -->
```

When syncing an existing translation to a new source commit, update both `Last Synced Commit` and `Last Updated`.

---

## What to Translate

Translate all descriptive text into the target language:

- Section headings and subheadings
- Feature descriptions and bullet point text
- Prose paragraphs (installation instructions, architecture explanations, contributing guidelines, license text)
- Table cell content (Status column values, Default column values, Description columns)
- Navigation link text in the language switcher table (use the native name of each language)

---

## What NOT to Translate

The following content must remain in English exactly as it appears in the source:

**Code blocks** — Every code block (Rust, Python, Go, JavaScript, TOML, shell commands) stays verbatim. Do not translate variable names, string literals, comments inside code, or command flags.

**Technical terms used universally in English** — Keep the following terms untranslated:
- ACID, WAL, MVCC, B+Tree, FFI
- Feature flag names: `temporal-core`, `temporal-edge`, `subgraph`, `hypergraph`, `full-temporal`, `plugin`
- Crate names: `cypherlite-query`, `cypherlite-storage`, `cypherlite-core`, `cypherlite-ffi`
- Type names: `Null`, `Bool`, `Int64`, `Float64`, `String`, `Bytes`, `Array`
- Cypher keywords: `MATCH`, `CREATE`, `MERGE`, `SET`, `DELETE`, `RETURN`, `WHERE`, `WITH`, `ORDER BY`, `AT TIME`, `SNAPSHOT`, `HYPEREDGE`
- Trait names: `Send`, `Sync`
- Library names: `parking_lot`, `logos`, `PyO3`, `napi-rs`, `CGo`
- Plugin type names: `ScalarFunction`, `IndexPlugin`, `Serializer`, `Trigger`, `PluginRegistry`, `TemporalRef`, `SubgraphStore`

**Badge markdown** — Badge image URLs and link URLs stay identical.

**File paths and directory names** — `examples/`, `bindings/`, `crates/`, `target/release/`, `.cyl`, `.cyl-wal`, `Cargo.toml`, `CONTRIBUTING.md`, `LICENSE-MIT`, `LICENSE-APACHE`.

**Package manager commands** — `cargo`, `pip`, `npm`, `go get`, `npx`, `maturin` commands remain in English.

**Version numbers** — `v0.1`, `v1.2`, `"1.2"`, `1.84`, `18+`, `1.21+`.

**GitHub URLs and links** — All URLs stay unchanged.

---

## Translation Quality Guidelines

**Natural fluency over literal accuracy.** A translation should read naturally to a native speaker, not like a word-for-word rendering of English. Restructure sentences when needed.

**Technical accuracy over literary quality.** When in doubt, favor the technically precise rendering. A user should be able to follow instructions from the translated file without referring to the English source.

**Consistent terminology.** Once you choose how to translate a term, use that same translation throughout the file. Refer to the Terminology Glossary section below.

**RTL languages (Arabic).** For Arabic (`ar`), the prose flows right-to-left as expected. Code blocks remain left-to-right; do not insert RTL markers inside fenced code blocks.

**Devanagari script (Hindi).** Use standard Devanagari Unicode for all Hindi text. Technical abbreviations (ACID, WAL) remain in Latin script.

**Bengali script.** Use standard Bengali Unicode. Technical abbreviations remain in Latin script.

---

## Terminology Glossary

Key terms and their established translations across the supported languages:

| English | zh | hi | es | fr | ar | bn | pt | ru | ko |
|---------|----|----|----|----|----|----|----|----|-----|
| Graph database | 图数据库 | ग्राफ़ डेटाबेस | base de datos de grafos | base de données graphe | قاعدة بيانات رسومية | গ্রাফ ডেটাবেস | banco de dados de grafos | графовая база данных | 그래프 데이터베이스 |
| Node | 节点 | नोड | nodo | nœud | عقدة | নোড | nó | узел | 노드 |
| Edge | 边 | एज | arista | arête | حافة | এজ | aresta | ребро | 에지 |
| Property | 属性 | प्रॉपर्टी | propiedad | propriété | خاصية | প্রপার্টি | propriedade | свойство | 속성 |
| Transaction | 事务 | ट्रांज़ैक्शन | transacción | transaction | معاملة | ট্রানজেকশন | transação | транзакция | 트랜잭션 |
| Storage engine | 存储引擎 | स्टोरेज इंजन | motor de almacenamiento | moteur de stockage | محرك التخزين | স্টোরেজ ইঞ্জিন | motor de armazenamento | движок хранения | 스토리지 엔진 |
| Query engine | 查询引擎 | क्वेरी इंजन | motor de consultas | moteur de requêtes | محرك الاستعلام | কোয়েরি ইঞ্জিন | motor de consultas | движок запросов | 쿼리 엔진 |
| Embedded | 嵌入式 | एम्बेडेड | embebido | embarqué | مدمج | এমবেডেড | embarcado | встраиваемый | 임베디드 |
| Feature flag | 特性标志 | फ़ीचर फ्लैग | indicador de característica | indicateur de fonctionnalité | علم الميزة | ফিচার ফ্ল্যাগ | sinalizador de recurso | флаг функции | 기능 플래그 |
| Plugin | 插件 | प्लगइन | complemento | module | إضافة | প্লাগইন | extensão | плагин | 플러그인 |
| Snapshot | 快照 | स्नैपशॉट | instantánea | instantané | لقطة | স্ন্যাপশট | instantâneo | снимок | 스냅샷 |
| Crash recovery | 崩溃恢复 | क्रैश रिकवरी | recuperación ante fallos | récupération après crash | استعادة بعد انهيار | ক্র্যাশ রিকভারি | recuperação após falha | восстановление после сбоя | 크래시 복구 |
| Throughput | 吞吐量 | थ्रूपुट | rendimiento | débit | إنتاجية | থ্রুপুট | throughput | пропускная способность | 처리량 |
| Coverage | 覆盖率 | कवरेज | cobertura | couverture | تغطية | কভারেজ | cobertura | покрытие | 커버리지 |
| Benchmark | 基准测试 | बेंचमार्क | prueba de rendimiento | benchmark | اختبار أداء | বেঞ্চমার্ক | benchmark | тест производительности | 벤치마크 |
| Hyperedge | 超边 | हाइपरएज | hiperarista | hyperarête | حافة فائقة | হাইপারএজ | hiperaresta | гиперребро | 하이퍼에지 |
| Subgraph | 子图 | सबग्राफ | subgrafo | sous-graphe | رسم بياني فرعي | সাবগ্রাফ | subgrafo | подграф | 서브그래프 |
| Rollback | 回滚 | रोलबैक | reversión | retour arrière | تراجع | রোলব্যাক | reversão | откат | 롤백 |
| Static library | 静态库 | स्टैटिक लाइब्रेरी | biblioteca estática | bibliothèque statique | مكتبة ساكنة | স্ট্যাটিক লাইব্রেরি | biblioteca estática | статическая библиотека | 정적 라이브러리 |
| Roadmap | 路线图 | रोडमैप | hoja de ruta | feuille de route | خارطة الطريق | রোডম্যাপ | roteiro | дорожная карта | 로드맵 |

---

## How to Check if a Translation is Outdated

A translation is outdated when the English `README.md` has changed since the commit recorded in `Last Synced Commit`.

To check:

```bash
# Show commits that changed the English README since the translation was last synced
git log --oneline <last-synced-commit>..HEAD -- README.md
```

If the output is non-empty, the translation needs updating.

To find the commit hash to record:

```bash
git rev-parse HEAD
```

After updating the translated file, set `Last Synced Commit` to that hash and `Last Updated` to today's date.

---

## PR Workflow for Translation Updates

1. Fork the repository and create a branch named `i18n/<code>/<description>`, for example `i18n/fr/sync-v1.2`.

2. Make changes only to the translation file you are updating (`docs/i18n/README.<code>.md`). Do not modify `README.md` or other translation files in the same PR unless coordinating a multi-language sync.

3. Update the metadata header with the new `Last Synced Commit` hash and today's `Last Updated` date.

4. Verify that:
   - The file opens with the metadata comment block
   - All section headings match the English source structure
   - No code blocks have been modified
   - Feature flag names, crate names, and type names are in English
   - Table row counts match the English source

5. Open a pull request with a title following this pattern: `i18n(fr): sync README to v1.2`

6. In the PR description, list the sections that changed and describe the nature of the updates (new section, corrected terminology, reworded paragraph).

---

## Adding a New Language

1. Check that an ISO 639-1 code exists for the language at https://en.wikipedia.org/wiki/List_of_ISO_639-1_codes.

2. Create `docs/i18n/README.<code>.md` based on the English source.

3. Add the full metadata header at the top.

4. Translate all descriptive text following the rules in this guide.

5. Add the language to the "Available in" link table in every existing translation file and in `README.md`. Use the native language name as the link text.

6. Add the language to the Terminology Glossary table above.

7. Open a PR with the title `i18n(<code>): add <Language Name> translation`.

---

## Questions

Open an [issue](https://github.com/Epsilondelta-ai/CypherLite/issues) with the label `i18n` if you have questions about translation scope, terminology choices, or the review process.
