import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  AppSnapshot,
  SyncReport,
  compactNumber,
  formatBytes,
  formatDate,
  getState,
  relativeTime,
  setIntervalMinutes,
  syncNow,
} from "./api";
import ActsView from "./ActsView";
import EmailView from "./EmailView";
import ExplorerView from "./ExplorerView";
import { Button, Message, Preload, Select } from "./components";
import logo from "./cwb-logo.svg";

/** Cadências oferecidas no seletor de atualização automática. */
const INTERVAL_CHOICES = [15, 30, 60, 180, 720];

type Page = "atos" | "explorador" | "email";

export default function App() {
  const [page, setPage] = useState<Page>("atos");
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Incrementado a cada atualização, para a tela visível refazer a consulta.
  const [refreshToken, setRefreshToken] = useState(0);
  const [progress, setProgress] = useState<Progress | null>(null);
  const tabsRef = useRef<HTMLDivElement>(null);

  const refresh = useCallback(async () => {
    try {
      setSnapshot(await getState());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // O backend atualiza sozinho em segundo plano; o resultado entra na tela.
  useEffect(() => {
    const unlisten = [
      listen<SyncReport>("sync-complete", () => {
        setProgress(null);
        setRefreshToken((n) => n + 1);
        refresh();
      }),
      listen<string>("sync-started", () => {
        setSnapshot((s) => (s ? { ...s, syncing: true } : s));
      }),
      listen<Progress>("ingest-progress", (e) => setProgress(e.payload)),
    ];
    return () => {
      unlisten.forEach((p) => p.then((f) => f()));
    };
  }, [refresh]);

  // Mantém os tempos relativos ("há 2 minutos") sempre atuais.
  const [, forceTick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => forceTick((n) => n + 1), 30_000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent("app:focus-search"));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const onSync = async () => {
    setError(null);
    setSnapshot((s) => (s ? { ...s, syncing: true } : s));
    try {
      await syncNow();
    } catch (e) {
      setError(String(e));
    } finally {
      setProgress(null);
      setRefreshToken((n) => n + 1);
      refresh();
    }
  };

  const onInterval = async (minutes: number) => {
    try {
      await setIntervalMinutes(minutes);
      setSnapshot((s) => (s ? { ...s, interval_minutes: minutes } : s));
    } catch (e) {
      setError(String(e));
    }
  };

  const summary = snapshot?.summary;
  const lastSync = snapshot?.last_sync;
  const report = snapshot?.act_report;
  const syncing = snapshot?.syncing ?? false;

  const status = useMemo(() => {
    if (lastSync?.error) return { tone: "bad" as const, text: lastSync.error };
    if (syncing) return { tone: "busy" as const, text: progressText(progress) };
    if (report) {
      const delta =
        report.action === "unchanged"
          ? "sem alterações"
          : report.action === "appended"
          ? `+${report.rows_added.toLocaleString("pt-BR")} atos novos`
          : `${report.total_rows.toLocaleString("pt-BR")} atos recarregados`;
      return {
        tone: "ok" as const,
        text: `Atualizado ${relativeTime(lastSync?.at ?? null)} · ${delta}`,
      };
    }
    if (lastSync) {
      return {
        tone: "ok" as const,
        text: `Verificado ${relativeTime(lastSync.at)}`,
      };
    }
    return { tone: "idle" as const, text: "Ainda não sincronizado" };
  }, [lastSync, syncing, report, progress]);

  /**
   * A primeira carga é uma tela própria, não uma tabela vazia.
   *
   * Sem nenhum ato guardado, o acervo ainda está sendo lido: mostrar a tela de
   * atos aqui daria uma tabela vazia com "nenhum ato" enquanto 1,4 GB desciam,
   * o que lê como erro e não como espera. A tabela só entra quando há dados ou
   * quando a leitura terminou sem eles (aí a mensagem de vazio é verdadeira).
   */
  const preparing =
    (report?.total_rows ?? 0) === 0 && (syncing || !lastSync);
  const prep = preparationCopy(progress);
  const progressPct =
    progress && progress.total_bytes > 0
      ? (progress.bytes / progress.total_bytes) * 100
      : null;

  const tabs: { id: Page; label: string }[] = [
    { id: "atos", label: "Atos" },
    { id: "explorador", label: "Explorador rsync" },
    { id: "email", label: "Email automático" },
  ];

  // Setas movem o foco entre as abas, como manda o padrão de tablist. Percorre a
  // lista pela posição (e não por um par fixo), para valer com qualquer número
  // de abas e envolver o fim.
  const onTabKey = (e: React.KeyboardEvent) => {
    if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
    e.preventDefault();
    const dir = e.key === "ArrowRight" ? 1 : -1;
    const i = tabs.findIndex((t) => t.id === page);
    setPage(tabs[(i + dir + tabs.length) % tabs.length].id);
    tabsRef.current?.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus();
  };

  return (
    <div className="app">
      <header className="br-header" role="banner">
        {/*
          Um único contêiner dentro do cabeçalho, envolvendo `.header-top` e
          `.header-bottom`: é o que o design system espera (`.br-header` é
          `display: flex`, e os dois blocos seriam duas colunas se ficassem
          soltos). `container-fluid` e não `container-lg`: o segundo limita a
          faixa a 1200px e, numa janela de aplicativo, deixaria o conteúdo
          flutuando no meio com sobra dos dois lados.
        */}
        <div className="container-fluid">
          <div className="header-top">
            <div className="header-logo">
              {/*
                A marca ocupa o lugar do ícone genérico que havia aqui. Como o
                arquivo é um SVG de traço fino, ele fica em `img` — e não
                inline — para não repetir 780 kB de caminhos no bundle JS: o
                arquivo é servido uma vez e ainda entra no cache do webview.
              */}
              <img className="brand-mark" src={logo} alt="" />
              <div className="header-sign">
                <div className="app-title">Legisladoc Sync</div>
                <div className="app-subtitle">
                  Base de atos da Prefeitura de Curitiba
                </div>
              </div>
            </div>
            <div className="header-actions">
              <div className={`status status-${status.tone}`}>
                <span className="status-dot" aria-hidden="true" />
                <span className="status-text" title={status.text}>
                  {status.text}
                </span>
              </div>
            </div>
          </div>

          <div className="header-bottom">
            {/*
              O design system desenha as abas em `.br-tab .tab-nav ul`: é o
              `<ul>` que recebe o flex e a linha inferior, e o `<nav>` é o
              `.tab-nav`.
            */}
            <div className="br-tab small" ref={tabsRef} onKeyDown={onTabKey}>
              <nav className="tab-nav" role="tablist" aria-label="Telas">
                <ul>
                  {tabs.map((t) => (
                    <li
                      key={t.id}
                      className={
                        page === t.id ? "tab-item is-active" : "tab-item"
                      }
                    >
                      <button
                        role="tab"
                        aria-selected={page === t.id}
                        onClick={() => setPage(t.id)}
                      >
                        <span className="name">{t.label}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </nav>
            </div>
            <div className="hero-controls">
              <Select
                id="intervalo"
                label="Intervalo de atualização automática"
                value={String(snapshot?.interval_minutes ?? 60)}
                disabled={!snapshot}
                onChange={(v) => onInterval(Number(v))}
                options={INTERVAL_CHOICES.map((m) => ({
                  value: String(m),
                  label: `Automático: ${m < 60 ? `${m} min` : `${m / 60} h`}`,
                }))}
              />
                <Button
                  emphasis="primary"
                  onClick={onSync}
                  loading={syncing}
                  disabled={syncing}
                >
                  {syncing ? "Atualizando…" : "Atualizar agora"}
                </Button>
            </div>
          </div>
        </div>
      </header>

      {/*
        O resumo é do acervo da origem (o que o rsync baixa), não dos atos já
        indexados: ele pertence à aba do explorador, e nas outras telas só
        ocuparia a altura que a tabela precisa.
      */}
      {page === "explorador" && (
        <section className="hero" aria-label="Resumo do acervo">
          {/*
            Quatro números do acervo. Nenhum é um gráfico: cada um é uma
            quantidade isolada, que se lê melhor como número grande com rótulo
            do que como barra. Os valores usam numerais tabulares para não
            dançarem de largura quando a atualização os muda.
          */}
          <div className="tiles">
            <Tile
              label="Atos publicados"
              value={report ? report.total_rows.toLocaleString("pt-BR") : "—"}
              note={
                report?.action === "appended" && report.rows_added > 0 ? (
                  <span className="delta-good">
                    +{report.rows_added.toLocaleString("pt-BR")} nesta atualização
                  </span>
                ) : (
                  "no acervo local"
                )
              }
            />
            <Tile
              label="Arquivos na origem"
              value={summary ? summary.total_files.toLocaleString("pt-BR") : "—"}
              note={
                summary
                  ? `mais recente em ${formatDate(summary.newest_mtime)}`
                  : "consultando a origem…"
              }
            />
            <Tile
              label="Volume publicado"
              value={
                summary
                  ? formatBytes(summary.listed_bytes || summary.total_bytes)
                  : "—"
              }
              note="somando todos os arquivos"
            />
            <Tile
              label="Alterados em 1 dia"
              value={summary ? summary.recent_count.toLocaleString("pt-BR") : "—"}
              note="arquivos com data recente"
            />
          </div>
          <div className="source" title={snapshot?.source}>
            {snapshot?.source ?? ""}
          </div>
        </section>
      )}

      {preparing && !(error || lastSync?.error) ? (
        <Preload
          title={prep.title}
          note={prep.note}
          percent={progressPct}
          detail={
            progress && progress.rows > 0
              ? `${compactNumber(progress.rows)} registros lidos`
              : undefined
          }
        />
      ) : (
        <>
          {(error || lastSync?.error) && (
            <div className="app-message">
              <Message kind="danger" title="Falha na atualização.">
                {error ?? lastSync?.error} — os dados exibidos são da última
                cópia válida.
              </Message>
            </div>
          )}

          {page === "atos" && <ActsView refreshToken={refreshToken} />}
          {page === "explorador" && (
            <ExplorerView refreshToken={refreshToken} syncing={syncing} />
          )}
          {page === "email" && <EmailView refreshToken={refreshToken} />}
        </>
      )}
    </div>
  );
}


/**
 * Uma quantidade do acervo: número grande, rótulo acima, nota abaixo.
 *
 * Não é um gráfico — é uma medida isolada, e a forma de leitura é o próprio
 * número. A nota é texto secundário: nunca carrega sozinha um significado que
 * dependa de cor.
 */
function Tile({
  label,
  value,
  note,
}: {
  label: string;
  value: string;
  note: React.ReactNode;
}) {
  return (
    <div className="tile">
      <div className="tile-label">{label}</div>
      <div className="tile-value">{value}</div>
      <div className="tile-note">{note}</div>
    </div>
  );
}

interface Progress {
  phase: string;
  rows: number;
  bytes: number;
  total_bytes: number;
}

/**
 * Título e nota da tela de preparação, por fase da ingestão.
 *
 * A primeira carga é a única que baixa os 1,4 GB inteiros; depois disso só a
 * parte nova desce. Por isso a fase `loading` fala em "primeira carga".
 */
function preparationCopy(p: Progress | null): { title: string; note: string } {
  switch (p?.phase) {
    case "checking":
      return {
        title: "Verificando a origem",
        note: "Conferindo se o arquivo de atos mudou desde a última leitura.",
      };
    case "schema":
      return {
        title: "Lendo os campos",
        note: "As definições de cada campo vêm do próprio feed e montam os índices de busca.",
      };
    case "loading":
      return {
        title: "Baixando e indexando os atos",
        note: "A primeira carga lê o acervo inteiro da origem e o guarda no banco local.",
      };
    case "tail":
      return {
        title: "Buscando os atos novos",
        note: "Só a parte acrescentada ao arquivo desde a última leitura.",
      };
    case "reloading":
      return {
        title: "Recarregando o acervo",
        note: "A origem foi reescrita, então os atos são lidos por inteiro de novo.",
      };
    default:
      return {
        title: "Preparando os dados",
        note: "Lendo o acervo da origem. Na primeira execução isso leva alguns minutos.",
      };
  }
}

/** Converte o progresso da ingestão em algo que se possa ler. */
function progressText(p: Progress | null): string {
  if (!p) return "Atualizando…";
  switch (p.phase) {
    case "checking":
      return "Verificando se o arquivo mudou…";
    case "schema":
      return "Lendo as definições dos campos…";
    case "tail":
      return "Baixando os registros novos…";
    case "reloading":
      return "Baixando o arquivo completo…";
    case "loading": {
      const pct = p.total_bytes
        ? Math.floor((p.bytes / p.total_bytes) * 100)
        : 0;
      return `Carregando atos… ${compactNumber(p.rows)} registros (${pct}%)`;
    }
    default:
      return "Atualizando…";
  }
}
