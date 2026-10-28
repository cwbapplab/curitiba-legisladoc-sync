/**
 * O painel de detalhes de um ato.
 *
 * Vive num módulo próprio porque duas telas o abrem: a lista de atos e a
 * simulação das regras. Mostra todos os campos que o feed publica, com os
 * rótulos do próprio `Info.xml`, e isola o cabeçalho e o texto do ato — que são
 * HTML — em quadros sem scripts.
 */
import { useEffect, useRef, useState } from "react";
import {
  ActDetail,
  actDetail,
  formatFeedValue,
  getEmailSettings,
  sendActEmail,
} from "./api";
import { Button, Icon, Input, Loading, Message } from "./components";

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

/** Todos os campos de um ato, com os rótulos que o próprio feed publica. */
export function DetailDrawer({ id, onClose }: { id: number; onClose: () => void }) {
  const [detail, setDetail] = useState<ActDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const closeRef = useRef<HTMLButtonElement>(null);

  // O envio avulso deste ato: para quem vai, e o que aconteceu.
  const [to, setTo] = useState("");
  const [sending, setSending] = useState(false);
  const [mailError, setMailError] = useState<string | null>(null);
  const [mailOk, setMailOk] = useState<string | null>(null);

  useEffect(() => {
    let vivo = true;
    setDetail(null);
    setError(null);
    setMailError(null);
    setMailOk(null);
    actDetail(id)
      .then((d) => vivo && setDetail(d))
      .catch((e) => vivo && setError(String(e)));
    return () => {
      vivo = false;
    };
  }, [id]);

  // Sugere o remetente configurado como destinatário; o campo continua livre
  // para outro endereço, e uma falha aqui não impede o envio.
  useEffect(() => {
    let vivo = true;
    getEmailSettings()
      .then((s) => {
        if (vivo && s.from_email) {
          setTo((atual) => (atual.trim() ? atual : s.from_email));
        }
      })
      .catch(() => {});
    return () => {
      vivo = false;
    };
  }, []);

  const enviar = async () => {
    if (!to.trim() || sending) return;
    setSending(true);
    setMailError(null);
    setMailOk(null);
    try {
      await sendActEmail(id, to);
      setMailOk("E-mail enviado.");
    } catch (e) {
      setMailError(String(e));
    } finally {
      setSending(false);
    }
  };

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
          {/*
            Envio avulso do ato aberto, um ato por vez. A mensagem sai montada
            como a das regras automáticas, então o destinatário a lê do mesmo
            jeito; o que muda é só o destinatário, escolhido aqui.
          */}
          <form
            className="form-field drawer-mail"
            onSubmit={(e) => {
              e.preventDefault();
              enviar();
            }}
          >
            <span className="tile-label">Enviar este ato por e-mail</span>
            <div className="password-row">
              <Input
                id="ato-email-dest"
                type="email"
                value={to}
                onChange={setTo}
                label="Destinatários"
                placeholder="um@exemplo.com; outro@exemplo.com"
              />
              <Button
                type="submit"
                emphasis="primary"
                loading={sending}
                disabled={!to.trim() || !detail}
              >
                <Icon name="mail" /> Enviar
              </Button>
            </div>
            {mailError && <Message kind="danger">{mailError}</Message>}
            {mailOk && (
              <Message kind="success" onClose={() => setMailOk(null)}>
                {mailOk}
              </Message>
            )}
          </form>

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
                            <span>{formatFeedValue(f.value)}</span>
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
