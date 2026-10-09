import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ActDetail,
  ActQuery,
  ActRow,
  Facet,
  Filter,
  Op,
  QueryCapabilities,
  StoreStats,
  actDetail,
  actFacets,
  actStats,
  compactNumber,
  formatActDate,
  queryCapabilities,
  searchActs,
} from "./api";
import { Button, Icon, Input, Loading, Message, Select, Tag } from "./components";

const PAGE_SIZE = 100;

/**
 * Colunas oferecidas como filtro, na ordem em que importam para quem lê atos.
 * Cada uma precisa estar na lista que o backend aceita; a interseção é
 * calculada, não presumida.
 */
const FACET_COLUMNS = [
  "TIPO_ATO_ID",
  "ORGAO_ID__ORIGEM",
  "SITUACAO_ACAO_ID",
  "ANO_PUBLICACAO",
];

type SortKey =
  | "ID"
  | "DATA_PUBLICACAO"
  | "DATA_CRIACAO"
  | "ANO_PUBLICACAO"
  | "NUMERO_OFICIAL";

interface Column {
  key: SortKey;
  label: string;
  className: string;
  /** Colunas de número são alinhadas à direita. */
  numeric?: boolean;
}

const COLUMNS: Column[] = [
  { key: "ID", label: "Identificação", className: "col-ident" },
  {
    key: "NUMERO_OFICIAL",
    label: "Nº oficial",
    className: "col-oficial",
    numeric: true,
  },
  {
    key: "DATA_PUBLICACAO",
    label: "Publicação",
    className: "col-data",
    numeric: true,
  },
  { key: "ANO_PUBLICACAO", label: "Ano", className: "col-ano", numeric: true },
];

interface Props {
  /** Alterado pelo shell após uma atualização, para a tela reconsultar. */
  refreshToken: number;
}

export default function ActsView({ refreshToken }: Props) {
  const [text, setText] = useState("");
  const [debounced, setDebounced] = useState("");
  const [filters, setFilters] = useState<Filter[]>([]);
  const [sort, setSort] = useState<SortKey>("DATA_PUBLICACAO");
  const [ascending, setAscending] = useState(false);
  const [offset, setOffset] = useState(0);

  const [rows, setRows] = useState<ActRow[]>([]);
  const [total, setTotal] = useState(0);
  const [stats, setStats] = useState<StoreStats | null>(null);
  const [caps, setCaps] = useState<QueryCapabilities | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [openId, setOpenId] = useState<number | null>(null);
  const searchRef = useRef<HTMLInputElement | null>(null);

  // Espera o usuário parar de digitar antes de consultar o banco.
  useEffect(() => {
    const t = setTimeout(() => setDebounced(text), 200);
    return () => clearTimeout(t);
  }, [text]);

  // As capacidades e os totais só mudam quando o acervo muda, então carregam
  // com a tela e a cada atualização — não a cada tecla digitada.
  useEffect(() => {
    let vivo = true;
    Promise.all([queryCapabilities(), actStats()])
      .then(([c, s]) => {
        if (!vivo) return;
        setCaps(c);
        setStats(s);
      })
      .catch((e) => vivo && setError(String(e)));
    return () => {
      vivo = false;
    };
  }, [refreshToken]);

  // Qualquer mudança na consulta volta para a primeira página; senão a página 4
  // do resultado anterior seria pedida contra o novo.
  useEffect(() => {
    setOffset(0);
  }, [debounced, filters, sort, ascending]);

  useEffect(() => {
    let vivo = true;
    const query: ActQuery = {
      text: debounced || null,
      filters,
      sort,
      ascending,
      limit: PAGE_SIZE,
      offset,
    };
    setLoading(true);
    searchActs(query)
      .then((r) => {
        if (!vivo) return;
        setRows(r.rows);
        setTotal(r.total);
        setError(null);
      })
      .catch((e) => vivo && setError(String(e)))
      .finally(() => vivo && setLoading(false));
    return () => {
      vivo = false;
    };
  }, [debounced, filters, sort, ascending, offset, refreshToken]);

  // Ctrl+F em qualquer lugar da janela leva o foco para a busca.
  useEffect(() => {
    const focus = () => searchRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        focus();
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("app:focus-search", focus);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("app:focus-search", focus);
    };
  }, []);

  const setFilter = useCallback((column: string, value: string) => {
    setFilters((fs) => {
      const outros = fs.filter((f) => f.column !== column);
      return value ? [...outros, { column, op: "eq" as Op, value }] : outros;
    });
  }, []);

  const toggleSort = (key: SortKey) => {
    if (key === sort) {
      setAscending((a) => !a);
    } else {
      setSort(key);
      setAscending(false);
    }
  };

  const pages = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const page = Math.floor(offset / PAGE_SIZE) + 1;

  const facetColumns = useMemo(
    () => (caps ? FACET_COLUMNS.filter((c) => caps.filters.includes(c)) : []),
    [caps]
  );

  const label = useCallback(
    (column: string) => caps?.labels.find(([c]) => c === column)?.[1] ?? column,
    [caps]
  );

  const ativo = Boolean(debounced) || filters.length > 0;

  return (
    <div className="view">
      <div className="toolbar">
        <InputWrapper inputRef={searchRef}>
          <Input
            id="busca-atos"
            type="search"
            value={text}
            onChange={setText}
            label="Buscar no texto dos atos"
            placeholder="Buscar no texto integral dos atos…  (Ctrl+F)"
            icon="search"
          />
        </InputWrapper>
        <div className="count" role="status" aria-live="polite">
          {loading
            ? "Buscando…"
            : `${total.toLocaleString("pt-BR")} ${
                total === 1 ? "ato" : "atos"
              }${ativo ? " encontrados" : ""}`}
        </div>
        {loading && <Loading inline label="Buscando atos" />}
      </div>

      <div className="filters" role="group" aria-label="Filtros de atos">
        {facetColumns.map((column) => (
          <FacetSelect
            key={column}
            column={column}
            label={label(column)}
            value={filters.find((f) => f.column === column)?.value ?? ""}
            onChange={(v) => setFilter(column, v)}
          />
        ))}
        {filters.length > 0 && (
          <Button onClick={() => setFilters([])}>Limpar filtros</Button>
        )}
      </div>

      {error && (
        <div className="app-message">
          <Message kind="danger" title="Falha na consulta.">
            {error}
          </Message>
        </div>
      )}

      <div className="table-wrap">
        <table className="app-table">
          <caption className="sr-only">
            Atos publicados. As colunas Identificação, Nº oficial, Publicação e
            Ano podem ser ordenadas.
          </caption>
          <thead>
            <tr>
              {COLUMNS.map((c) => (
                <th
                  key={c.key}
                  className={c.numeric ? `${c.className} head-num` : c.className}
                  aria-sort={
                    sort === c.key
                      ? ascending
                        ? "ascending"
                        : "descending"
                      : "none"
                  }
                >
                  <button
                    className="sort-btn"
                    onClick={() => toggleSort(c.key)}
                    disabled={!caps?.sorts.includes(c.key)}
                  >
                    {c.label}
                    <span className="sort-mark" aria-hidden="true">
                      {sort === c.key ? (ascending ? "▲" : "▼") : ""}
                    </span>
                  </button>
                </th>
              ))}
              <th className="col-tipo">Tipo</th>
              <th className="col-orgao">Órgão</th>
              <th className="col-sumula">Ementa</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr
                key={r.id}
                tabIndex={0}
                className={openId === r.id ? "selected" : undefined}
                onClick={() => setOpenId(r.id)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    setOpenId(r.id);
                  }
                }}
              >
                <td className="col-ident">
                  <div className="ident-cell">
                    <span className="ident" title={r.numero_identificacao ?? ""}>
                      {r.numero_identificacao ?? `#${r.id}`}
                    </span>
                    {r.situacao && <Tag>{r.situacao}</Tag>}
                  </div>
                </td>
                <td className="col-oficial num">{r.numero_oficial ?? "—"}</td>
                <td className="col-data num">
                  {formatActDate(r.data_publicacao)}
                </td>
                <td className="col-ano num">{r.ano_publicacao ?? "—"}</td>
                <td className="col-tipo ellipsis" title={r.tipo ?? ""}>
                  {r.tipo ?? "—"}
                </td>
                <td className="col-orgao ellipsis" title={r.orgao ?? ""}>
                  {r.orgao ?? "—"}
                </td>
                <td className="col-sumula">
                  {r.snippet ? (
                    // O trecho traz `<mark>` do `snippet()` do FTS5 — o único
                    // HTML do feed que pode ser renderizado.
                    <Snippet html={r.snippet} />
                  ) : (
                    <span className="sumula" title={r.sumula ?? ""}>
                      {r.sumula ?? "—"}
                    </span>
                  )}
                </td>
              </tr>
            ))}
            {!loading && rows.length === 0 && (
              <tr className="empty-row">
                <td colSpan={COLUMNS.length + 3}>
                  {stats?.total === 0
                    ? "Nenhum ato armazenado ainda — a primeira carga está em andamento ou falhou."
                    : ativo
                    ? "Nenhum ato atende a esses critérios."
                    : "Nenhum ato armazenado."}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <footer className="foot">
        <span>
          {stats
            ? `${compactNumber(stats.total)} atos · ${
                stats.newest_publication
                  ? `publicação mais recente em ${formatActDate(
                      stats.newest_publication
                    )}`
                  : "sem publicações"
              }`
            : "Carregando…"}
        </span>
        <span className="spacer" />
        {total > PAGE_SIZE && (
          <nav className="br-pagination small" aria-label="Paginação">
            <ul>
              <li>
                <Button
                  circle
                  density="small"
                  ariaLabel="Página anterior"
                  disabled={page <= 1}
                  onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
                >
                  <Icon name="chevron-left" />
                </Button>
              </li>
              <li>
                <span className="page active" aria-current="page">
                  {page}
                  <span className="sr-only"> de {pages}</span>
                </span>
              </li>
              <li>
                <Button
                  circle
                  density="small"
                  ariaLabel="Próxima página"
                  disabled={page >= pages}
                  onClick={() => setOffset(offset + PAGE_SIZE)}
                >
                  <Icon name="chevron-right" />
                </Button>
              </li>
            </ul>
          </nav>
        )}
      </footer>

      {openId !== null && (
        <DetailDrawer id={openId} onClose={() => setOpenId(null)} />
      )}
    </div>
  );
}

/**
 * Repassa a referência do campo de busca para o `Input` do design system, que
 * encapsula o próprio `<input>`.
 */
function InputWrapper({
  children,
  inputRef,
}: {
  children: React.ReactNode;
  inputRef: React.RefObject<HTMLInputElement | null>;
}) {
  const holder = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = holder.current?.querySelector("input");
    if (el) inputRef.current = el;
  }, [inputRef]);
  return <div ref={holder}>{children}</div>;
}

/**
 * Lista suspensa com os valores distintos de uma coluna, e suas contagens.
 *
 * As contagens são da coluna inteira, não do resultado atual: elas dizem o que
 * existe no acervo, que é o que torna um valor digno de ser oferecido.
 */
function FacetSelect({
  column,
  label,
  value,
  onChange,
}: {
  column: string;
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const [facets, setFacets] = useState<Facet[] | null>(null);

  useEffect(() => {
    let vivo = true;
    actFacets(column)
      .then((f) => vivo && setFacets(f))
      .catch(() => vivo && setFacets([]));
    return () => {
      vivo = false;
    };
  }, [column]);

  return (
    <div className="facet">
      <span className="tile-label" id={`rotulo-${column}`}>
        {label}
      </span>
      <Select
        id={`filtro-${column}`}
        label={label}
        value={value}
        onChange={onChange}
        disabled={!facets || facets.length === 0}
        options={[
          { value: "", label: "Todos" },
          ...(facets ?? []).map((f) => ({
            value: f.value,
            label: `${f.value} (${f.count.toLocaleString("pt-BR")})`,
          })),
        ]}
      />
    </div>
  );
}

/**
 * Um campo cujo valor é HTML, mostrado como HTML.
 *
 * É o caso de `CABECALHO`: o feed publica ali o cabeçalho do documento, com
 * `<p>`, `<strong>` e — nos documentos gerados no Word — até um bloco `<style>`
 * com a formatação. Não é texto, então não pode ir para um `<dd>`: ali as tags
 * apareceriam cruas e ainda estilizariam o resto da aplicação. Vai para um
 * quadro isolado, sem scripts e sem mesma origem, como o texto do ato.
 *
 * A altura vem de uma estimativa, não de uma medida: `sandbox=""` bloqueia
 * também os scripts do próprio quadro, então ele não tem como informar a altura
 * do conteúdo — e liberar scripts só para medi-la deixaria de bloquear os
 * scripts do feed. Contar os blocos do cabeçalho basta para acertar a ordem de
 * grandeza, e o pouco que sobra rola dentro do quadro.
 */
function HtmlField({ title, value }: { title: string; value: string | null }) {
  if (!value) return null;

  // Blocos de texto — parágrafos, quebras, divisores — e não marcações de
  // formatação, que não ocupam linha própria.
  const blocks = value.match(/<\/?(p|div|br|hr|h[1-6]|table|tr|li)\b/gi)?.length ?? 1;
  // 220px acomoda os blocos de cabeçalho do Word, que têm margens generosas.
  const height = Math.min(70 + blocks * 22, 260);

  return (
    <section className="body-section">
      <h3>{title}</h3>
      <iframe
        className="act-body short"
        title={title}
        sandbox=""
        style={{ height }}
        srcDoc={value}
      />
    </section>
  );
}

/** Marca interna e improvável no texto, para poupar o destaque da limpeza. */
const MARK_OPEN = "\u0001O\u0001";
const MARK_CLOSE = "\u0001C\u0001";
const MARK_SPLIT = new RegExp(`(${MARK_OPEN}|${MARK_CLOSE})`);

/**
 * Renderiza um trecho do FTS5 deixando passar apenas o `<mark>` que o backend
 * emite. O texto vem do feed, então é escapado parte por parte em vez de ser
 * tratado como HTML.
 *
 * A ordem importa: o destaque é trocado por uma marca interna, as marcações do
 * feed são removidas e só então as entidades são resolvidas. Decodificar antes
 * transformaria um `&lt;mark&gt;` escrito no ato num destaque que ninguém pediu.
 */
function Snippet({ html }: { html: string }) {
  const stripped = stripHtml(
    html.replace(/<mark>/g, MARK_OPEN).replace(/<\/mark>/g, MARK_CLOSE)
  );

  let marcado = false;
  return (
    <span className="sumula">
      {stripped.split(MARK_SPLIT).map((p, i) => {
        if (p === MARK_OPEN) {
          marcado = true;
          return null;
        }
        if (p === MARK_CLOSE) {
          marcado = false;
          return null;
        }
        const texto = decodeEntities(p);
        return marcado ? <mark key={i}>{texto}</mark> : <span key={i}>{texto}</span>;
      })}
    </span>
  );
}

/**
 * Entidades que aparecem de fato nos atos, onde os diagramadores escreveram um
 * `&nbsp;` ou o texto veio de um documento formatado.
 */
const ENTITIES: Record<string, string> = {
  amp: "&",
  lt: "<",
  gt: ">",
  quot: '"',
  apos: "'",
  nbsp: " ",
  ndash: "–",
  mdash: "—",
  hellip: "…",
  ldquo: "“",
  rdquo: "”",
  lsquo: "‘",
  rsquo: "’",
  ordm: "º",
};

/** Casa uma entidade nomeada, decimal ou hexadecimal. */
const ENTITY_RE = /&(#[0-9]{1,7}|#[xX][0-9a-fA-F]{1,6}|[a-zA-Z][a-zA-Z0-9]{1,31});/g;

/**
 * `&amp;` etc. → o caractere que representam.
 *
 * O trecho vem do valor bruto do feed, que é HTML: sem isto a célula mostraria
 * `&amp;` onde o ato diz `&`. O que não for reconhecido fica como está — melhor
 * mostrar `&algumacoisa;` do que sumir com o texto.
 */
function decodeEntities(text: string): string {
  return text.replace(ENTITY_RE, (whole, body: string) => {
    if (body[0] === "#") {
      const hex = body[1] === "x" || body[1] === "X";
      const code = parseInt(hex ? body.slice(2) : body.slice(1), hex ? 16 : 10);
      // Fora da faixa dos caracteres válidos, `fromCodePoint` lança; o texto
      // original é a resposta segura.
      return Number.isFinite(code) && code > 0 && code <= 0x10ffff
        ? String.fromCodePoint(code)
        : whole;
    }
    return ENTITIES[body.toLowerCase()] ?? whole;
  });
}

/**
 * Remove as marcações de HTML do trecho, preservando o `<mark>` do FTS5.
 *
 * O `snippet()` do FTS5 é montado a partir do valor bruto da coluna que melhor
 * casou, e o índice cobre `CABECALHO` e `body`, cujo conteúdo é HTML. Sem esta
 * limpeza a coluna Ementa mostraria `<br>`, `&nbsp;` e — pior — pedaços de CSS
 * no meio da frase.
 *
 * Os `<script>` e `<style>` saem inteiros, com o conteúdo: são justamente as
 * partes que não são texto do ato, e deixá-los faria a célula exibir regras de
 * estilo. O quadro pode começar no meio de uma marcação (o trecho é cortado em
 * qualquer ponto), então uma tag sem o `>` inicial também é descartada.
 */
function stripHtml(html: string): string {
  return (
    html
      .replace(/<script\b[\s\S]*?<\/script\s*>/gi, " ")
      .replace(/<style\b[\s\S]*?<\/style\s*>/gi, " ")
      .replace(/<[^>]*>/g, " ")
      // Um trecho cortado no meio de uma tag: `<br`, ou o fim de um atributo
      // como ` margin-right:0cm">`, que o `replace` acima não alcança porque
      // não tem o `<`. Só se corta até o fim, onde não há mais texto do ato.
      .replace(/^[^<>]*>\s*/, "")
      // A tag aberta no fim do quadro fica pendurada; vai junto com ela.
      .replace(/<[^>]*$/, " ")
      .replace(/\s+/g, " ")
      .trim()
  );
}

/** Todos os campos de um ato, com os rótulos que o próprio feed publica. */
function DetailDrawer({ id, onClose }: { id: number; onClose: () => void }) {
  const [detail, setDetail] = useState<ActDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    let vivo = true;
    setDetail(null);
    actDetail(id)
      .then((d) => vivo && setDetail(d))
      .catch((e) => vivo && setError(String(e)));
    return () => {
      vivo = false;
    };
  }, [id]);

  useEffect(() => {
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const fields = detail?.fields ?? [];
  const identificacao = fields.find((f) =>
    f.label.toLowerCase().includes("identificação")
  );

  return (
    <>
      <div className="drawer-scrim" onClick={onClose} />
      <aside
        className="drawer"
        role="dialog"
        aria-modal="true"
        aria-label="Detalhes do ato"
      >
        <header className="drawer-head">
          <div>
            <div className="drawer-title">
              {identificacao?.value ?? fields[0]?.value ?? `Ato ${id}`}
            </div>
            <div className="drawer-sub">Registro nº {id}</div>
          </div>
          <span ref={closeRef as unknown as React.RefObject<HTMLSpanElement>}>
            <Button circle ariaLabel="Fechar" onClick={onClose}>
              <Icon name="close" />
            </Button>
          </span>
        </header>

        <div className="drawer-body">
          {error && (
            <div className="app-message">
              <Message kind="danger">{error}</Message>
            </div>
          )}
          {!detail && !error && <Loading label="Carregando o ato" />}
          {detail && (
            <>
              <dl className="fields">
                {fields
                  // `body` e `CABECALHO` são os dois campos cujo conteúdo é
                  // HTML, não texto — são quadros próprios abaixo, não linhas
                  // desta lista.
                  .filter((f) => f.column !== "body" && f.column !== "CABECALHO")
                  .map((f) => (
                    <div className="field" key={f.column}>
                      <dt title={f.column}>{f.label}</dt>
                      <dd>
                        {f.value ? (
                          <>
                            <span>{f.value}</span>
                            {f.related_value && f.related_value !== f.value && (
                              <span className="resolved">
                                {f.related_value}
                              </span>
                            )}
                          </>
                        ) : (
                          <span className="muted">—</span>
                        )}
                      </dd>
                    </div>
                  ))}
              </dl>

              <HtmlField
                title="Cabeçalho"
                value={
                  fields.find((f) => f.column === "CABECALHO")?.value ?? null
                }
              />

              {detail.body && (
                <section className="body-section">
                  <h3>Texto do ato</h3>
                  {/*
                    O texto do ato é HTML vindo do feed. É renderizado num
                    quadro isolado, sem scripts e sem mesma origem, para que uma
                    tag de script ou um acesso à janela matriz não alcance a
                    ponte de comandos da aplicação.
                  */}
                  <iframe
                    className="act-body"
                    title="Texto do ato"
                    sandbox=""
                    srcDoc={detail.body}
                  />
                </section>
              )}
            </>
          )}
        </div>
      </aside>
    </>
  );
}
