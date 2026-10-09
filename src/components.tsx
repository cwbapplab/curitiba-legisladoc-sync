import { ReactNode } from "react";

/**
 * Peças do GOVBR-DS usadas em mais de uma tela.
 *
 * A biblioteca publica duas formas: os *web components* (`<br-button>`, que
 * trazem estilos encapsulados em shadow DOM e só aceitam `::part`) e o pacote
 * de CSS por classes (`@govbr-ds/core`). A tabela de atos tem sete colunas com
 * alinhamentos e larguras próprios, que um componente encapsulado não
 * permitiria ajustar, então a aplicação usa as **classes** do design system em
 * toda parte — a mesma marcação que a documentação descreve, escrita à mão.
 */

type ButtonEmphasis = "primary" | "secondary" | "tertiary";

interface ButtonProps {
  children: ReactNode;
  emphasis?: ButtonEmphasis;
  /** `small` para ações dentro de linhas e rodapés. */
  density?: "small" | "medium";
  /** Só ícone: exige `aria-label`, senão o botão fica sem nome acessível. */
  circle?: boolean;
  type?: "button" | "submit";
  disabled?: boolean;
  loading?: boolean;
  onClick?: () => void;
  ariaLabel?: string;
}

export function Button({
  children,
  emphasis = "secondary",
  density,
  circle,
  type = "button",
  disabled,
  loading,
  onClick,
  ariaLabel,
}: ButtonProps) {
  const classes = ["br-button", emphasis];
  if (density === "small") classes.push("small");
  if (circle) classes.push("circle");
  if (loading) classes.push("loading");
  return (
    <button
      type={type}
      className={classes.join(" ")}
      disabled={disabled || loading}
      onClick={onClick}
      aria-label={ariaLabel}
      aria-busy={loading || undefined}
    >
      {children}
    </button>
  );
}

interface LoadingProps {
  /** Anunciado por leitores de tela enquanto o conteúdo não chega. */
  label?: string;
  inline?: boolean;
}

export function Loading({ label = "Carregando", inline }: LoadingProps) {
  return (
    <span
      className={inline ? "br-loading inline" : "br-loading"}
      role="status"
      aria-live="polite"
    >
      <span className="sr-only">{label}</span>
    </span>
  );
}

/**
 * Tela de preparação do acervo.
 *
 * Só aparece quando não há nenhum ato guardado e a origem ainda está sendo
 * lida — a primeira execução, ou logo após apagar o banco local. Antes disso a
 * tela de atos ficava vazia enquanto 1,4 GB desciam, sem dizer nada; aqui o
 * carregamento é o próprio conteúdo.
 *
 * O anel é o `br-loading` do design system com `data-progress`: ele desenha o
 * percentual e a fatia preenchida. Como o design system não tem um componente
 * de barra de progresso nesta versão, a barra fina abaixo é feita com os tokens
 * dele — mesma cor de interação, mesma altura de traço.
 */
export function Preload({
  title,
  note,
  percent,
  detail,
}: {
  title: string;
  note: string;
  /** 0–100. `null` enquanto o total ainda é desconhecido (fase de verificação). */
  percent: number | null;
  detail?: string;
}) {
  const value = percent === null ? null : Math.max(0, Math.min(100, Math.round(percent)));
  return (
    <div className="preload" role="status" aria-live="polite">
      <div className="preload-card">
        {value === null ? (
          <span className="br-loading medium preload-ring" />
        ) : (
          <span
            className="br-loading preload-ring"
            // O anel do design system lê o percentual deste atributo: é ele que
            // escolhe a fatia preenchida e desenha o número no centro. O
            // `aria-valuenow` sozinho não basta — o CSS só olha `data-progress`.
            data-progress={value}
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={value}
            aria-label={`${title} ${value}%`}
          >
            <span className="br-loading-mask full">
              <span className="br-loading-fill" />
            </span>
            <span className="br-loading-mask">
              <span className="br-loading-fill" />
            </span>
          </span>
        )}

        <h1 className="preload-title">{title}</h1>
        <p className="preload-note">{note}</p>

        <div
          className="preload-track"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={value ?? undefined}
        >
          <div
            className={value === null ? "preload-bar indeterminate" : "preload-bar"}
            style={{ width: value === null ? "100%" : `${value}%` }}
          />
        </div>

        {detail && <p className="preload-detail">{detail}</p>}
      </div>
    </div>
  );
}

type MessageKind = "info" | "success" | "warning" | "danger";

/**
 * `br-message` — avisos e mensagens de estado.
 *
 * O ícone é decorativo: o significado já está no texto, e uma cor sozinha não
 * seria acessível.
 */
export function Message({
  kind = "info",
  title,
  children,
  onClose,
}: {
  kind?: MessageKind;
  title?: string;
  children: ReactNode;
  onClose?: () => void;
}) {
  return (
    <div className="br-message" role={kind === "danger" ? "alert" : "status"}>
      <div className="icon">
        <Icon name={MESSAGE_ICON[kind]} />
      </div>
      <div className="content">
        {/*
          Título e corpo são blocos, não spans: o design system não lhes dá
          margem nem `display`, então em linha os dois textos colariam um no
          outro ("Falha na atualização.TypeError…").
        */}
        {title && <div className="message-title">{title}</div>}
        <div className="message-body">{children}</div>
      </div>
      {onClose && (
        <div className="close">
          <button
            className="br-button circle small"
            onClick={onClose}
            aria-label="Fechar"
          >
            <Icon name="close" />
          </button>
        </div>
      )}
    </div>
  );
}

const MESSAGE_ICON: Record<MessageKind, IconName> = {
  info: "info",
  success: "check-circle",
  warning: "exclamation",
  danger: "exclamation",
};

/**
 * `br-input` — campo de texto, com um ícone à esquerda quando pedido.
 *
 * O rótulo é sempre renderizado (inclusive quando invisível) porque o design
 * system o posiciona em cima do campo e sem ele a marcação perde o vínculo
 * entre rótulo e controle.
 */
export function Input({
  id,
  value,
  onChange,
  placeholder,
  label,
  type = "text",
  icon,
}: {
  id: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  label: string;
  type?: "text" | "search";
  icon?: IconName;
}) {
  return (
    <div className={icon ? "br-input has-icon" : "br-input"}>
      <label className="sr-only" htmlFor={id}>
        {label}
      </label>
      {icon && (
        <div className="input-icon">
          <Icon name={icon} />
        </div>
      )}
      <input
        id={id}
        type={type === "search" ? "search" : "text"}
        value={value}
        placeholder={placeholder}
        spellCheck={false}
        autoComplete="off"
        onChange={(e) => onChange(e.target.value)}
      />
    </div>
  );
}

/**
 * `br-select` — lista suspensa.
 *
 * O design system prevê um componente com busca e lista própria; aqui os
 * conjuntos de valores são pequenos e já chegam contados do banco, então o
 * elemento nativo é usado com as mesmas classes visuais.
 */
export function Select({
  id,
  value,
  onChange,
  label,
  options,
  disabled,
}: {
  id: string;
  value: string;
  onChange: (value: string) => void;
  label: string;
  options: { value: string; label: string }[];
  disabled?: boolean;
}) {
  return (
    <div className="br-select">
      <label className="sr-only" htmlFor={id}>
        {label}
      </label>
      <select
        id={id}
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}

/** `br-divider` — separador de seções. */
export function Divider({ vertical }: { vertical?: boolean }) {
  return (
    <hr
      className={vertical ? "br-divider vertical" : "br-divider"}
      aria-hidden="true"
    />
  );
}

/** `br-tag` — etiqueta curta: situação do ato, marcador de novidade. */
export function Tag({
  children,
  status,
}: {
  children: ReactNode;
  status?: "success" | "warning" | "danger" | "info";
}) {
  const classes = ["br-tag"];
  if (status) classes.push(status);
  return <span className={classes.join(" ")}>{children}</span>;
}

export type IconName =
  | "search"
  | "close"
  | "info"
  | "check-circle"
  | "exclamation"
  | "chevron-left"
  | "chevron-right"
  | "refresh"
  | "file";

/**
 * Ícones em SVG inline.
 *
 * O design system espera Font Awesome por classe (`fas fa-search`), que
 * exigiria carregar uma fonte externa — algo que uma aplicação de desktop sem
 * rede não pode assumir. Os traços são os mesmos e herdam a cor do texto.
 */
export function Icon({ name }: { name: IconName }) {
  const paths: Record<IconName, ReactNode> = {
    search: (
      <>
        <circle cx="11" cy="11" r="7" />
        <line x1="16.5" y1="16.5" x2="21" y2="21" />
      </>
    ),
    close: (
      <>
        <line x1="6" y1="6" x2="18" y2="18" />
        <line x1="18" y1="6" x2="6" y2="18" />
      </>
    ),
    info: (
      <>
        <circle cx="12" cy="12" r="9" />
        <line x1="12" y1="11" x2="12" y2="16.5" />
        <line x1="12" y1="7.5" x2="12" y2="7.6" />
      </>
    ),
    "check-circle": (
      <>
        <circle cx="12" cy="12" r="9" />
        <path d="M8 12.5l2.5 2.5L16 9.5" />
      </>
    ),
    exclamation: (
      <>
        <path d="M12 3.5L21 20H3z" />
        <line x1="12" y1="10" x2="12" y2="15" />
        <line x1="12" y1="17.5" x2="12" y2="17.6" />
      </>
    ),
    "chevron-left": <path d="M15 5l-7 7 7 7" />,
    "chevron-right": <path d="M9 5l7 7-7 7" />,
    refresh: (
      <>
        <path d="M20 12a8 8 0 1 1-2.6-5.9" />
        <path d="M20 4v5h-5" />
      </>
    ),
    file: (
      <>
        <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
        <path d="M14 3v5h5" />
      </>
    ),
  };

  return (
    <svg
      viewBox="0 0 24 24"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      {paths[name]}
    </svg>
  );
}
