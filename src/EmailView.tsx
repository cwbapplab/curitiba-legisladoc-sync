import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ColumnValue,
  EmailLogEntry,
  EmailRule,
  EmailRuleInput,
  EmailSettings,
  EmailSettingsInput,
  EmailStatus,
  Filter,
  Logic,
  Op,
  QueryCapabilities,
  actColumnValues,
  deleteEmailRule,
  formatDate,
  getEmailSettings,
  getEmailStatus,
  listEmailLog,
  listEmailRules,
  queryCapabilities,
  runEmailRulesNow,
  saveEmailRule,
  saveEmailSettings,
  sendTestEmail,
  testEmailConnection,
} from "./api";
import {
  Button,
  Divider,
  Icon,
  Input,
  Loading,
  Message,
  MultiSelect,
  Select,
  Tag,
  Toggle,
} from "./components";

/** Rótulos dos operadores, iguais aos que o backend aceita. */
const OPS: { value: Op; label: string }[] = [
  { value: "eq", label: "é igual a" },
  { value: "ne", label: "é diferente de" },
  { value: "like", label: "contém" },
  { value: "gte", label: "maior ou igual a" },
  { value: "lte", label: "menor ou igual a" },
];

interface Props {
  refreshToken: number;
}

/**
 * A tela de notificações.
 *
 * Duas ideias convivem aqui: **regras** (um filtro salvo, com destinatários e
 * um liga/desliga) e a **configuração de SMTP**, que fica atrás da engrenagem.
 * As regras são aplicadas aos atos que uma sincronização acabou de acrescentar;
 * se alguma casar, o e-mail sai.
 */
export default function EmailView({ refreshToken }: Props) {
  const [rules, setRules] = useState<EmailRule[] | null>(null);
  const [status, setStatus] = useState<EmailStatus | null>(null);
  const [log, setLog] = useState<EmailLogEntry[]>([]);
  const [caps, setCaps] = useState<QueryCapabilities | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [editing, setEditing] = useState<EmailRule | "new" | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [r, s, l, c] = await Promise.all([
        listEmailRules(),
        getEmailStatus(),
        listEmailLog(30),
        queryCapabilities(),
      ]);
      setRules(r);
      setStatus(s);
      setLog(l);
      setCaps(c);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load, refreshToken]);

  const toggleRule = async (rule: EmailRule) => {
    setRules((rs) =>
      rs ? rs.map((r) => (r.id === rule.id ? { ...r, enabled: !r.enabled } : r)) : rs
    );
    try {
      await saveEmailRule({
        id: rule.id,
        name: rule.name,
        enabled: !rule.enabled,
        text: rule.text,
        filters: rule.filters,
        recipients: rule.recipients.join("; "),
      });
    } catch (e) {
      setError(String(e));
      load(); // desfaz a alteração otimista
    }
  };

  const removeRule = async (rule: EmailRule) => {
    if (!confirm(`Excluir a regra “${rule.name}”?`)) return;
    try {
      await deleteEmailRule(rule.id);
      load();
    } catch (e) {
      setError(String(e));
    }
  };

  const runNow = async (send: boolean) => {
    setBusy(true);
    setNotice(null);
    try {
      const entries = await runEmailRulesNow(send, false);
      setLog(entries);
      const total = entries.reduce((n, e) => n + (e.matched || 0), 0);
      setNotice(
        send
          ? `Execução concluída: ${total} ato(s) casaram. Veja o histórico abaixo.`
          : `Simulação: ${total} ato(s) casariam. Nada foi enviado.`
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="view">
      <div className="toolbar">
        <Button emphasis="primary" onClick={() => setEditing("new")}>
          <Icon name="plus" /> Nova regra
        </Button>
        <Button onClick={() => runNow(false)} disabled={busy}>
          Simular
        </Button>
        <Button onClick={() => runNow(true)} disabled={busy}>
          Enviar agora
        </Button>
        {busy && <Loading inline label="Executando regras" />}
        <span className="spacer" />
        <div className="email-status" role="status" aria-live="polite">
          {status
            ? `${status.enabled_rules} regra(s) ativa(s) · ${status.act_count.toLocaleString(
                "pt-BR"
              )} atos no acervo`
            : "Carregando…"}
        </div>
        <Button
          circle
          ariaLabel="Configurar servidor de e-mail"
          onClick={() => setSettingsOpen(true)}
        >
          <Icon name="cog" />
        </Button>
      </div>

      <p className="email-hint">
        As regras são aplicadas aos atos <strong>novos</strong> de cada
        atualização automática. Se um ato recém-publicado casar com o filtro de
        uma regra ativa, os detalhes dele são enviados por e-mail. A primeira
        carga não envia nada.
      </p>

      {error && (
        <div className="app-message">
          <Message kind="danger" title="Falha.">
            {error}
          </Message>
        </div>
      )}
      {notice && (
        <div className="app-message">
          <Message kind="success" onClose={() => setNotice(null)}>
            {notice}
          </Message>
        </div>
      )}

      <div className="table-wrap">
        <table className="app-table">
          <caption className="sr-only">
            Regras de notificação por e-mail. Cada regra tem um filtro,
            destinatários e um interruptor.
          </caption>
          <thead>
            <tr>
              <th className="col-rule-name">Regra</th>
              <th className="col-rule-cond">Condição</th>
              <th className="col-rule-dest">Destinatários</th>
              <th className="col-rule-on">Ativa</th>
              <th className="col-rule-actions">Ações</th>
            </tr>
          </thead>
          <tbody>
            {(rules ?? []).map((r) => (
              <tr key={r.id}>
                <td className="col-rule-name">
                  <span className="ident" title={r.name}>
                    {r.name}
                  </span>
                </td>
                <td className="col-rule-cond">
                  <ConditionSummary rule={r} caps={caps} />
                </td>
                <td className="col-rule-dest ellipsis" title={r.recipients.join("; ")}>
                  {r.recipients.join("; ")}
                </td>
                <td className="col-rule-on">
                  <Toggle
                    id={`regra-ativa-${r.id}`}
                    checked={r.enabled}
                    onChange={() => toggleRule(r)}
                    label=""
                  />
                </td>
                <td className="col-rule-actions">
                  <Button density="small" onClick={() => setEditing(r)}>
                    Editar
                  </Button>
                  <Button
                    circle
                    density="small"
                    ariaLabel={`Excluir regra ${r.name}`}
                    onClick={() => removeRule(r)}
                  >
                    <Icon name="trash" />
                  </Button>
                </td>
              </tr>
            ))}
            {rules && rules.length === 0 && (
              <tr className="empty-row">
                <td colSpan={5}>
                  Nenhuma regra ainda. Use <strong>Nova regra</strong> para
                  escolher um filtro e os destinatários, e configure o servidor
                  pela engrenagem.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <section className="email-log" aria-label="Histórico de notificações">
        <h3 className="section-title">Histórico</h3>
        {log.length === 0 ? (
          <p className="muted">Nenhuma notificação registrada ainda.</p>
        ) : (
          <ul>
            {log.map((e, i) => (
              <li key={i}>
                <span className="email-log-time">{formatDate(e.at)}</span>
                <Tag status={STATUS_TAG[e.status] ?? "info"}>
                  {STATUS_LABEL[e.status] ?? e.status}
                </Tag>
                <span className="email-log-rule">{e.rule_name}</span>
                <span className="muted">
                  {e.matched > 0 ? `${e.matched} ato(s)` : "sem resultados"}
                </span>
                {e.error && <span className="danger-text">{e.error}</span>}
              </li>
            ))}
          </ul>
        )}
      </section>

      {editing && caps && (
        <RuleDrawer
          rule={editing === "new" ? null : editing}
          caps={caps}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            setNotice("Regra salva.");
            load();
          }}
        />
      )}

      {settingsOpen && (
        <SettingsDrawer onClose={() => setSettingsOpen(false)} />
      )}
    </div>
  );
}

const STATUS_TAG: Record<string, "success" | "warning" | "danger" | "info"> = {
  sent: "success",
  failed: "danger",
  skipped: "info",
};

const STATUS_LABEL: Record<string, string> = {
  sent: "enviado",
  failed: "falhou",
  skipped: "sem envio",
};

/** Traduz os filtros de uma regra em uma frase legível. */
function ConditionSummary({
  rule,
  caps,
}: {
  rule: EmailRule;
  caps: QueryCapabilities | null;
}) {
  const label = (column: string) =>
    caps?.labels.find(([c]) => c === column)?.[1] ?? column;
  const op = (o: Op) => OPS.find((x) => x.value === o)?.label ?? o;

  // Os valores extras entram na frase como "“A” ou “B”" — é assim que a
  // condição realmente casa, já que vários valores são um "é um destes".
  const parts = rule.filters.map((f) => {
    const all = [f.value, ...(f.values ?? [])].filter((v) => v.trim());
    const quoted =
      all.length === 0
        ? "…"
        : all.length === 1
        ? `“${all[0]}”`
        : all.map((v) => `“${v}”`).join(" ou ");
    return `${label(f.column)} ${op(f.op)} ${quoted}`;
  });
  // O termo de texto é uma condição "e" a mais, antes de todas as outras.
  if (rule.text) parts.unshift(`texto contém “${rule.text}”`);

  // Cada condição a partir da segunda vem precedida do seu conector; "e" é o
  // padrão do banco, e escrevê-lo entre todas as condições poluiria a frase sem
  // informar nada. O deslocamento desconta o termo de texto, que entra como uma
  // condição "e" a mais e não tem conector próprio.
  const offset = rule.text ? 1 : 0;
  const phrase = parts
    .map((p, i) => {
      const condition = i - offset;
      // A primeira condição não se liga a nada — o `logic` dela é ignorado pelo
      // banco, então também não é mostrado.
      if (condition < 1) return p;
      return rule.filters[condition]?.logic === "or" ? `ou ${p}` : p;
    })
    .join(" · ");

  return (
    <span className="ellipsis" title={parts.join(" · ")}>
      {parts.length ? phrase : "Todos os atos novos"}
    </span>
  );
}

/* --------------------------------------------------------------- edit rule */

/**
 * Uma condição da regra.
 *
 * A coluna decide o resto: quando `Info.xml` define um tipo para ela, o valor
 * vem de uma lista (um enum ou uma tabela de domínio) e o campo de texto vira
 * uma escolha múltipla; quando não define, continua livre. É por isso que o
 * componente busca a lista ao montar e a cada troca de coluna — a consulta é
 * local e barata, e mantém o valor oferecido igual ao que o banco aceita.
 *
 * O conector (e/ou) fica na mesma linha da condição, à esquerda, mas só existe
 * a partir da segunda: a primeira não tem com o que se ligar.
 */
function ConditionRow({
  index,
  filter,
  columns,
  labelOf,
  onChange,
  onRemove,
}: {
  index: number;
  filter: Filter;
  columns: { value: string; label: string }[];
  labelOf: (column: string) => string;
  onChange: (patch: Partial<Filter>) => void;
  onRemove: () => void;
}) {
  const [defs, setDefs] = useState<ColumnValue[] | null>(null);

  useEffect(() => {
    let alive = true;
    if (!filter.column) {
      setDefs(null);
      return;
    }
    actColumnValues(filter.column)
      .then((v) => {
        if (alive) setDefs(v);
      })
      // Sem lista é o caso normal (coluna livre); a falha cai no mesmo lugar
      // que ele, ou seja, no campo de texto.
      .catch(() => {
        if (alive) setDefs(null);
      });
    return () => {
      alive = false;
    };
  }, [filter.column]);

  // Operadores de comparação ("contém", "maior", "menor") não têm lista: uma
  // enumeração só descreve igualdade e diferença.
  const listOk = filter.op === "eq" || filter.op === "ne";
  const useList = listOk && !!defs && defs.length > 0;

  // Ao trocar de coluna o valor antigo não vale mais para a nova.
  const changeColumn = (column: string) => onChange({ column, value: "", values: [] });

  // Os operadores de comparação usam só o primeiro valor; ao entrar num deles,
  // os extras são descartados em vez de ficarem escondidos e ainda irem para o
  // banco na próxima gravação.
  const changeOp = (op: Op) =>
    onChange(op === "eq" || op === "ne" ? { op } : { op, values: [] });

  // A escolha múltipla trabalha com um vetor só; o primeiro vai para `value`,
  // que é o campo que o banco sempre teve, e o resto para `values`.
  const chosen = [filter.value, ...(filter.values ?? [])].filter((v) => v.trim());
  const setChosen = (all: string[]) => onChange({ value: all[0] ?? "", values: all.slice(1) });

  // Um valor guardado pode não estar mais na lista (a tabela de domínio não
  // veio no último *refresh*, por exemplo). Sem isto ele sumiria da tela e o
  // usuário salvaria uma regra que não vê.
  const options = useList
    ? [
        ...defs!.map((d) => ({ value: d.value, label: d.label })),
        ...chosen
          .filter((v) => !defs!.some((d) => d.value === v))
          .map((v) => ({ value: v, label: v })),
      ]
    : [];

  return (
    <div className="rule-filter">
      <div className="rule-logic">
        {index > 0 ? (
          <Select
            id={`filtro-logic-${index}`}
            label="Conector"
            value={filter.logic ?? "and"}
            onChange={(v) => onChange({ logic: v as Logic })}
            options={[
              { value: "and", label: "E" },
              { value: "or", label: "OU" },
            ]}
          />
        ) : (
          <span className="rule-logic-spacer" aria-hidden="true" />
        )}
      </div>

      <Select
        id={`filtro-col-${index}`}
        label="Coluna"
        value={filter.column}
        onChange={changeColumn}
        options={[
          { value: filter.column, label: labelOf(filter.column) },
          ...columns.filter((c) => c.value !== filter.column),
        ]}
      />
      <Select
        id={`filtro-op-${index}`}
        label="Operador"
        value={filter.op}
        onChange={(v) => changeOp(v as Op)}
        options={OPS}
      />

      <div className="rule-value">
        {useList ? (
          <MultiSelect
            id={`filtro-val-${index}`}
            label="Valor"
            options={options}
            values={chosen}
            onChange={setChosen}
          />
        ) : (
          <Input
            id={`filtro-val-${index}`}
            value={filter.value}
            onChange={(v) => onChange({ value: v, values: [] })}
            label="Valor"
            placeholder="Valor"
          />
        )}
      </div>

      <Button circle density="small" ariaLabel="Remover condição" onClick={onRemove}>
        <Icon name="close" />
      </Button>
    </div>
  );
}

function RuleDrawer({
  rule,
  caps,
  onClose,
  onSaved,
}: {
  rule: EmailRule | null;
  caps: QueryCapabilities;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [name, setName] = useState(rule?.name ?? "");
  const [enabled, setEnabled] = useState(rule?.enabled ?? true);
  const [text, setText] = useState(rule?.text ?? "");
  const [filters, setFilters] = useState<Filter[]>(rule?.filters ?? []);
  const [recipients, setRecipients] = useState(rule?.recipients.join("; ") ?? "");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  const columns = useMemo(
    () => caps.filters.map((c) => ({ value: c, label: labelOf(caps, c) })),
    [caps]
  );

  const setFilter = (i: number, patch: Partial<Filter>) =>
    setFilters((fs) => fs.map((f, j) => (j === i ? { ...f, ...patch } : f)));

  const save = async () => {
    setSaving(true);
    setError(null);
    const input: EmailRuleInput = {
      id: rule?.id ?? null,
      name,
      enabled,
      text: text.trim() || null,
      filters,
      recipients,
    };
    try {
      await saveEmailRule(input);
      onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <div className="drawer-scrim" onClick={onClose} />
      <aside
        className="drawer"
        role="dialog"
        aria-modal="true"
        aria-label={rule ? "Editar regra" : "Nova regra"}
      >
        <header className="drawer-head">
          <div>
            <div className="drawer-title">
              {rule ? "Editar regra" : "Nova regra"}
            </div>
            <div className="drawer-sub">
              O filtro é aplicado aos atos novos de cada atualização.
            </div>
          </div>
          <Button circle ariaLabel="Fechar" onClick={onClose}>
            <Icon name="close" />
          </Button>
        </header>

        <form
          className="drawer-body settings-form"
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          {error && <Message kind="danger">{error}</Message>}

          <Field label="Nome da regra">
            <Input
              id="regra-nome"
              value={name}
              onChange={setName}
              label="Nome da regra"
              placeholder="Ex.: Decretos da saúde"
            />
          </Field>

          <Field label="Buscar no texto integral (opcional)">
            <Input
              id="regra-texto"
              type="search"
              value={text}
              onChange={setText}
              label="Buscar no texto integral"
              placeholder="Termo que deve aparecer no ato"
              icon="search"
            />
          </Field>

          <Field label="Condições">
            <div className="rule-filters">
              {filters.map((f, i) => (
                <ConditionRow
                  key={i}
                  index={i}
                  filter={f}
                  columns={columns}
                  labelOf={(c) => labelOf(caps, c)}
                  onChange={(patch) => setFilter(i, patch)}
                  onRemove={() => setFilters((fs) => fs.filter((_, j) => j !== i))}
                />
              ))}
              <Button
                density="small"
                onClick={() =>
                  setFilters((fs) => [
                    ...fs,
                    {
                      column: columns[0]?.value ?? "",
                      op: "eq" as Op,
                      value: "",
                      // "e" é o padrão do banco; declarado aqui para a UI e o
                      // que ela salva nunca divergirem.
                      logic: "and" as Logic,
                    },
                  ])
                }
                disabled={columns.length === 0}
              >
                <Icon name="plus" /> Adicionar condição
              </Button>
              <p className="muted">
                Sem condições, a regra casa com <strong>todos</strong> os atos
                novos.
              </p>
            </div>
          </Field>

          <Field label="Destinatários">
            <Input
              id="regra-dest"
              type="email"
              value={recipients}
              onChange={setRecipients}
              label="Destinatários"
              placeholder="um@exemplo.com; outro@exemplo.com"
            />
          </Field>

          <Field label="Situação">
            <Toggle
              id="regra-enabled"
              checked={enabled}
              onChange={setEnabled}
              label="Regra ativa"
            />
          </Field>

          <Divider />

          <div className="form-actions">
            <Button onClick={onClose}>Cancelar</Button>
            <Button emphasis="primary" type="submit" loading={saving}>
              Salvar
            </Button>
          </div>
        </form>
      </aside>
    </>
  );
}

/* ----------------------------------------------------------- settings */

function SettingsDrawer({ onClose }: { onClose: () => void }) {
  const [settings, setSettings] = useState<EmailSettings | null>(null);
  const [form, setForm] = useState<EmailSettingsInput | null>(null);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [testTo, setTestTo] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    getEmailSettings()
      .then((s) => {
        setSettings(s);
        setForm({
          provider: s.provider,
          host: s.host,
          port: s.port,
          security: s.security,
          username: s.username,
          from_name: s.from_name,
          from_email: s.from_email,
        });
        setTestTo(s.from_email);
      })
      .catch((e) => setError(String(e)));
  }, []);

  const patch = (p: Partial<EmailSettingsInput>) =>
    setForm((f) => (f ? { ...f, ...p } : f));

  const google = form?.provider === "google";

  const save = async () => {
    if (!form) return;
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      const saved = await saveEmailSettings({
        ...form,
        password: password.trim() || null,
      });
      setSettings(saved);
      setPassword("");
      setInfo("Configuração salva.");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const test = async () => {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      await testEmailConnection();
      setInfo("Conexão e autenticação funcionaram.");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const sendTest = async () => {
    setBusy(true);
    setError(null);
    setInfo(null);
    try {
      await sendTestEmail(testTo);
      setInfo(`E-mail de teste enviado para ${testTo}.`);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="drawer-scrim" onClick={onClose} />
      <aside
        className="drawer"
        role="dialog"
        aria-modal="true"
        aria-label="Configuração de e-mail"
      >
        <header className="drawer-head">
          <div>
            <div className="drawer-title">Servidor de e-mail</div>
            <div className="drawer-sub">
              Por onde as notificações são enviadas.
            </div>
          </div>
          <Button circle ariaLabel="Fechar" onClick={onClose}>
            <Icon name="close" />
          </Button>
        </header>

        <form
          className="drawer-body settings-form"
          onSubmit={(e) => {
            e.preventDefault();
            save();
          }}
        >
          {error && <Message kind="danger">{error}</Message>}
          {info && <Message kind="success">{info}</Message>}
          {!form && <Loading label="Carregando configuração" />}

          {form && (
            <>
              <Field label="Provedor">
                <Select
                  id="smtp-provider"
                  label="Provedor"
                  value={form.provider}
                  onChange={(v) =>
                    patch({
                      provider: v as EmailSettingsInput["provider"],
                      // O preset do Google já traz host e porta prontos.
                      ...(v === "google"
                        ? { host: "smtp.gmail.com", port: 587, security: "starttls" }
                        : {}),
                    })
                  }
                  options={[
                    { value: "custom", label: "Servidor personalizado (SMTP)" },
                    { value: "google", label: "Google (Gmail)" },
                  ]}
                />
              </Field>

              {google ? (
                <Message kind="info" title="Senha de app do Google">
                  O Gmail não aceita a senha normal da conta. Ative a
                  verificação em duas etapas e crie uma{" "}
                  <strong>senha de app</strong> de 16 caracteres na sua conta
                  Google; use-a no campo de senha abaixo.
                </Message>
              ) : (
                <>
                  <Field label="Servidor (host)">
                    <Input
                      id="smtp-host"
                      value={form.host}
                      onChange={(v) => patch({ host: v })}
                      label="Servidor SMTP"
                      placeholder="smtp.exemplo.com"
                    />
                  </Field>
                  <div className="form-row">
                    <Field label="Porta">
                      <Input
                        id="smtp-port"
                        value={String(form.port)}
                        onChange={(v) => patch({ port: Number(v) || 0 })}
                        label="Porta"
                        placeholder="587"
                      />
                    </Field>
                    <Field label="Segurança">
                      <Select
                        id="smtp-security"
                        label="Segurança"
                        value={form.security}
                        onChange={(v) =>
                          patch({ security: v as EmailSettingsInput["security"] })
                        }
                        options={[
                          { value: "starttls", label: "STARTTLS (587)" },
                          { value: "tls", label: "TLS (465)" },
                          { value: "none", label: "Sem TLS (relay local)" },
                        ]}
                      />
                    </Field>
                  </div>
                </>
              )}

              <Field label="Usuário">
                <Input
                  id="smtp-user"
                  value={form.username}
                  onChange={(v) => patch({ username: v })}
                  label="Usuário"
                  placeholder="voce@exemplo.com"
                />
              </Field>

              <Field label={settings?.has_password ? "Senha (guardada)" : "Senha"}>
                <div className="password-row">
                  <Input
                    id="smtp-pass"
                    type={showPassword ? "text" : "password"}
                    value={password}
                    onChange={setPassword}
                    label="Senha"
                    autoComplete="new-password"
                    placeholder={
                      settings?.has_password
                        ? "•••••••• (deixe em branco para manter)"
                        : "senha de app"
                    }
                  />
                  <Button
                    circle
                    density="small"
                    ariaLabel={showPassword ? "Ocultar senha" : "Mostrar senha"}
                    onClick={() => setShowPassword((s) => !s)}
                  >
                    <Icon name={showPassword ? "eye-off" : "eye"} />
                  </Button>
                </div>
              </Field>

              <Divider />

              <Field label="Remetente (nome)">
                <Input
                  id="smtp-from-name"
                  value={form.from_name}
                  onChange={(v) => patch({ from_name: v })}
                  label="Nome do remetente"
                  placeholder="Legisladoc"
                />
              </Field>
              <Field label="Remetente (e-mail)">
                <Input
                  id="smtp-from-email"
                  type="email"
                  value={form.from_email}
                  onChange={(v) => patch({ from_email: v })}
                  label="E-mail do remetente"
                  placeholder="voce@exemplo.com"
                />
              </Field>

              <Divider />

              <Field label="Enviar e-mail de teste para">
                <div className="password-row">
                  <Input
                    id="smtp-test-to"
                    type="email"
                    value={testTo}
                    onChange={setTestTo}
                    label="Destinatário do teste"
                    placeholder="voce@exemplo.com"
                  />
                  <Button onClick={sendTest} disabled={busy || !testTo}>
                    <Icon name="mail" /> Testar envio
                  </Button>
                </div>
              </Field>

              <p className="muted">
                A senha é guardada cifrada no banco local do aplicativo. Isso
                protege contra a leitura casual do arquivo do banco (uma cópia
                do <code>.sqlite</code> não revela a senha), mas não contra
                alguém com acesso aos arquivos da sua conta de usuário, onde
                fica também a chave de decifração.
              </p>

              <div className="form-actions">
                <Button onClick={test} disabled={busy}>
                  Testar conexão
                </Button>
                <span className="spacer" />
                <Button onClick={onClose}>Cancelar</Button>
                <Button emphasis="primary" type="submit" loading={busy}>
                  Salvar
                </Button>
              </div>
            </>
          )}
        </form>
      </aside>
    </>
  );
}

/** Rótulo visível acima de um controle; o design system não traz um. */
function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="form-field">
      <span className="tile-label">{label}</span>
      {children}
    </div>
  );
}

function labelOf(caps: QueryCapabilities, column: string): string {
  return caps.labels.find(([c]) => c === column)?.[1] ?? column;
}
