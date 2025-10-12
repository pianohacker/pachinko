import { useCallback, useState } from "react";
import appClasses from "./App.module.css";
import { ItemList } from "./ItemList";
import { ErrorBoundary } from "./ErrorBoundary";

function App() {
  const [search, setSearch] = useState("");

  const onKeyDown: React.KeyboardEventHandler = useCallback((e) => {
    if (e.key == "Escape") setSearch("");
  }, []);

  return (
    <>
      <main className={appClasses.main} onKeyDown={onKeyDown}>
        <header className={appClasses.mainHeader}>
          <h1>Pachinko</h1>
          <input
            className={appClasses.search}
            type="search"
            placeholder="Search..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </header>
        <ErrorBoundary fallback={<p>Server error, please reload</p>}>
          <ItemList search={search} />
        </ErrorBoundary>
      </main>
    </>
  );
}

export default App;
