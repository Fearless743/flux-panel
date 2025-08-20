
import ReactDOM from "react-dom/client";
import { MemoryRouter } from "react-router-dom";

import App from "./App.tsx";
import { Provider } from "./provider.tsx";
import "@/styles/globals.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <MemoryRouter>
    <Provider>
      <App />
    </Provider>
  </MemoryRouter>
);

