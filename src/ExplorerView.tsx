import { useEffect, useRef, useState } from "react";
import {
  FileRow,
  SortBy,
  formatBytes,
  formatDate,
  searchFiles,
} from "./api";
import { Icon, Input, Loading, Message, Tag } from "./components";

const PAGE_SIZE = 500;

interface Column {
  key: SortBy;
  label: string;
  className: string;
  numeric?: boolean;
}

const COLUMNS: Column[] = [
  { key: "path", label: "Arquivo", className: "col-file" },
  { key: "size", label: "Tamanho", className: "col-size", numeric: true },
  {
    key: "mtime",
    label: "Modificado",
    className: "col-updated",
    numeric: true,
  },
];

interface Props {
  /** Alterado pelo shell após uma atualização, para a tela reconsultar. */
  refreshToken: number;
  syncing: boolean;
}

/**
 * A listagem crua do rsync.
 *
 * Fica numa tela separada em vez de misturada à visão de atos: é o retrato sem
 * tratamento do que o servidor publica — inclusive os arquivos cujo conteúdo
 * esta aplicação não lê — e é onde se verifica se o acervo local está velho.
 */
export default function ExplorerView({ refreshToken, syncing }: Props) {
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [rows, setRows] = useState<FileRow[]>([]);
  const [total, setTotal] = useState(0);
  const [sortBy, setSortBy] = useState<SortBy>("mtime");
  const [ascending, setAscending] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const searchRef = useRef<HTMLInputElement | null>(null);

  // Espera o usuário parar de digitar antes de consultar o banco.
  useEffect(() => {
    const t = setTimeout(() => setDebounced(query), 180);
    return () => clearTimeout(t);
  }, [query]);

  useEffect(() => {
    let vivo = true;
    setLoading(true);
    searchFiles(debounced, sortBy, ascending, PAGE_SIZE, 0)
      .then((page) => {
        if (!vivo) return;
        setRows(page.rows);
        setTotal(page.total);
        setError(null);
      })
      .catch((e) => vivo && setError(String(e)))
      .finally(() => vivo && setLoading(false));
    return () => {
      vivo = false;
    };
  }, [debounced, sortBy, ascending, refreshToken]);

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

  const toggleSort = (key: SortBy) => {
    if (key === sortBy) {
      setAscending((a) => !a);
    } else {
      setSortBy(key);
      setAscending(key === "path");
    }
  };

  return (
    <div className="view">
      <div className="toolbar">
        <div
          ref={(el) => {
            const input = el?.querySelector("input");
            if (input) searchRef.current = input;
          }}
        >
          <Input
            id="busca-arquivos"
            type="search"
            value={query}
            onChange={setQuery}
            label="Buscar nomes de arquivo"
            placeholder="Buscar nomes de arquivo…  (Ctrl+F)"
            icon="search"
          />
        </div>
        <div className="count" role="status" aria-live="polite">
          {loading
            ? "Carregando…"
            : `${total.toLocaleString("pt-BR")} ${
                total === 1 ? "arquivo" : "arquivos"
              }${query ? " encontrados" : ""}`}
        </div>
        {loading && <Loading inline label="Carregando arquivos" />}
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
            Arquivos publicados na origem. As colunas podem ser ordenadas.
          </caption>
          <thead>
            <tr>
              {COLUMNS.map((c) => (
                <th
                  key={c.key}
                  className={c.numeric ? `${c.className} head-num` : c.className}
                  aria-sort={
                    sortBy === c.key
                      ? ascending
                        ? "ascending"
                        : "descending"
                      : "none"
                  }
                >
                  <button
                    className="sort-btn"
                    onClick={() => toggleSort(c.key)}
                  >
                    {c.label}
                    <span className="sort-mark" aria-hidden="true">
                      {sortBy === c.key ? (ascending ? "▲" : "▼") : ""}
                    </span>
                  </button>
                </th>
              ))}
              {/* Não ordenável: a coluna é um estado, não um critério de busca. */}
              <th className="col-synced">Sincronizado</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr
                key={r.id}
                tabIndex={0}
                className={selected === r.path ? "selected" : undefined}
                onClick={() => setSelected(r.path)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") {
                    e.preventDefault();
                    setSelected(r.path);
                  }
                }}
              >
                <td className="col-file">
                  <div className="ident-cell">
                    <Icon name="file" />
                    <span className="ident" title={r.path}>
                      {r.path}
                    </span>
                    {r.recent && <Tag status="success">novo</Tag>}
                  </div>
                </td>
                <td className="col-size num">{formatBytes(r.size)}</td>
                <td className="col-updated num">{formatDate(r.mtime)}</td>
                <td className="col-synced">
                  {r.synced_at ? (
                    <Tag status="info">
                      sincronizado em {formatDate(r.synced_at)}
                    </Tag>
                  ) : (
                    <span className="muted" aria-label="não sincronizado">
                      —
                    </span>
                  )}
                </td>
              </tr>
            ))}
            {!loading && rows.length === 0 && (
              <tr className="empty-row">
                <td colSpan={COLUMNS.length + 1}>
                  {query
                    ? `Nenhum arquivo corresponde a “${query}”.`
                    : syncing
                    ? "Listando a origem pela primeira vez…"
                    : "Nenhum arquivo indexado ainda — use “Atualizar agora”."}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <footer className="foot">
        <span>{syncing ? "Relendo a origem…" : ""}</span>
        {selected && <span className="ellipsis">{selected}</span>}
        <span className="spacer" />
        <span>
          {rows.length < total
            ? `mostrando ${rows.length.toLocaleString("pt-BR")}`
            : ""}
        </span>
      </footer>
    </div>
  );
}
