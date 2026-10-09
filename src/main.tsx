import React from "react";
import ReactDOM from "react-dom/client";

// GOVBR-DS (Padrão Digital de Governo) — tokens, utilitários e componentes.
// `core-base.css` traz os tokens e os utilitários; cada componente mora em seu
// próprio arquivo, então os que a aplicação usa são importados um a um.
import "@govbr-ds/core/dist/core-base.css";
import "@govbr-ds/core/dist/components/button/button.css";
import "@govbr-ds/core/dist/components/input/input.css";
import "@govbr-ds/core/dist/components/select/select.css";
import "@govbr-ds/core/dist/components/table/table.css";
import "@govbr-ds/core/dist/components/tag/tag.css";
import "@govbr-ds/core/dist/components/message/message.css";
import "@govbr-ds/core/dist/components/tab/tab.css";
import "@govbr-ds/core/dist/components/header/header.css";
import "@govbr-ds/core/dist/components/loading/loading.css";
import "@govbr-ds/core/dist/components/divider/divider.css";
import "@govbr-ds/core/dist/components/card/card.css";
import "@govbr-ds/core/dist/components/pagination/pagination.css";
import "@govbr-ds/core/dist/components/scrim/scrim.css";
import "@govbr-ds/core/dist/components/modal/modal.css";

import App from "./App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
