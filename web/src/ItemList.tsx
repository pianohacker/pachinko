import {
  memo,
  Suspense,
  use,
  useCallback,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { fetchApi } from "./api";
import type { Item, ItemLocation } from "./types";

import itemClasses from "./ItemList.module.css";
import fuzzysort from "fuzzysort";
import {
  ModuleRegistry,
  ClientSideRowModelModule,
  ValidationModule,
  type ColDef,
  themeQuartz,
  CellStyleModule,
  TextEditorModule,
  SelectEditorModule,
  type CellEditingStoppedEvent,
  type RowClassParams,
  RowStyleModule,
  RenderApiModule,
  NumberEditorModule,
} from "ag-grid-community";
import { AgGridReact } from "ag-grid-react";

ModuleRegistry.registerModules([
  ClientSideRowModelModule,
  CellStyleModule,
  TextEditorModule,
  NumberEditorModule,
  SelectEditorModule,
  RowStyleModule,
  RenderApiModule,
]);
// via process.env.NODE_ENV
if (import.meta.env.VITE_ENV !== "production") {
  ModuleRegistry.registerModules([ValidationModule]);
}

type SearchResult = {
  item: Item;
  columns: string[];
  score: number;
  highlights: (() => string)[];
};

const compareResults = (a: SearchResult, b: SearchResult): number =>
  Math.round(b.score * 20) / 20 - Math.round(a.score * 20) / 20 ||
  a.item.location.name.localeCompare(b.item.location.name) ||
  a.item.bin_no - b.item.bin_no ||
  a.item.name.localeCompare(b.item.name);

const agGridTheme = themeQuartz.withParams({
  borderWidth: 0,
  cellHorizontalPadding: ".25rem",
  fontFamily: "Spectral",
  fontSize: "1.5rem",
});

const cellRenderer =
  (i: number) =>
  ({ data }: { data: SearchResult }) => {
    const highlighted = data.highlights[i]?.();

    return highlighted ? (
      <span dangerouslySetInnerHTML={{ __html: highlighted }} />
    ) : (
      data.columns[i]
    );
  };

const ItemsListInner = memo(
  ({
    itemsPromise,
    locationsPromise,
    search,
    setErrorMessage,
  }: {
    itemsPromise: Promise<Item[]>;
    locationsPromise: Promise<ItemLocation[]>;
    search: string;
    setErrorMessage: (x: string) => void;
  }) => {
    const items = use(itemsPromise);
    const locations = use(locationsPromise);

    const allResults: SearchResult[] = useMemo(() => {
      const results = items.map((item) => ({
        item,
        score: 1,
        columns: [item.location.name, item.bin_no.toString(), item.name],
        highlights: [],
      }));
      results.sort(compareResults);
      return results;
    }, [items]);

    const filteredResults: SearchResult[] = useMemo(() => {
      if (!search) return allResults;

      const results = fuzzysort
        .go(search, items, {
          keys: ["location.name", "bin_no", "name"],
        })
        .map((result) => ({
          item: result.obj,
          score: result.score,
          columns: [
            result.obj.location.name,
            result.obj.bin_no.toString(),
            result.obj.name,
          ],
          highlights: [
            result[0].highlight.bind(result[0]),
            result[1].highlight.bind(result[1]),
            result[2].highlight.bind(result[2]),
          ],
        }));
      results.sort(compareResults);
      return results;
    }, [items, allResults, search]);

    const colDefs = useMemo(
      (): ColDef<SearchResult>[] => [
        {
          colId: "location",
          cellClass: itemClasses.itemLocation,
          cellRenderer: cellRenderer(0),
          editable: true,
          cellEditor: "agSelectCellEditor",
          cellEditorParams: {
            values: locations.map(({ name }) => name),
          },
          valueGetter: ({ data }) => data?.columns[0],
          valueSetter: ({ data, newValue }) => {
            const newLocation = locations.find(({ name }) => name == newValue);
            if (!newLocation) return false;

            delete data.highlights[0];
            data.item.location = newLocation;
            data.columns[0] = newValue;
            return true;
          },
        },
        {
          valueGetter: () => "/",
          width: 10,
          cellClass: itemClasses.itemSlash,
        },
        {
          colId: "bin_no",
          width: 40,
          cellClass: itemClasses.itemBinNo,
          cellRenderer: cellRenderer(1),
          editable: true,
          cellEditorSelector: ({ data }) => ({
            component: "agNumberCellEditor",
            params: {
              min: 1,
              max: data.item.location.num_bins,
            },
          }),
          valueGetter: ({ data }) => data?.columns[1],
          valueSetter: ({ data, newValue }) => {
            const newBinNo = parseInt(newValue);
            if (!isNaN(newBinNo)) return false;

            delete data.highlights[1];
            data.item.bin_no = newBinNo;
            data.columns[1] = newBinNo.toString();
            return true;
          },
        },
        {
          colId: "name",
          flex: 1,
          cellRenderer: cellRenderer(2),
          editable: true,
          cellEditor: "agTextCellEditor",
          valueGetter: ({ data }) => data?.columns[2],
          valueSetter: ({ data, newValue }) => {
            const newName = (newValue as string).trim();
            if (!newName) return false;

            delete data.highlights[2];
            data.item.name = newValue;
            data.columns[2] = newValue;
            return true;
          },
        },
      ],
      [locations],
    );

    const getRowClass = useCallback(
      ({ data }: RowClassParams<SearchResult>) => {
        if (!data) return undefined;

        return data.score < 0.3 ? itemClasses.lowScore : undefined;
      },
      [],
    );

    const onCellEditingStopped = useCallback(
      async (event: CellEditingStoppedEvent<SearchResult>) => {
        if (!event.data || !event.valueChanged) return;
        const { item } = event.data;

        const update: { location_id?: number } & Partial<
          Pick<Item, "bin_no" | "name">
        > = {};

        switch (event.colDef.colId) {
          case "location":
            if (!item.location.object_id) return;

            update.location_id = item.location.object_id;
            break;
          case "bin_no":
            update.bin_no = item.bin_no;
            break;
          case "name":
            update.name = item.name;
            break;
        }

        try {
          await fetchApi(`/items/${item.object_id}`, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
            },
            body: JSON.stringify(update),
          });
        } catch (e) {
          console.error(`Failed to update item: ${e}`);
          setErrorMessage(`Failed to update item, please reload: ${e}`);
        }
      },
      [setErrorMessage],
    );

    const gridRef = useRef<AgGridReact<SearchResult> | null>(null);

    useEffect(() => {
      if (!gridRef.current?.api) return;

      gridRef.current.api.refreshCells({ force: true });
    }, [filteredResults]);

    return (
      <AgGridReact
        ref={gridRef}
        rowData={filteredResults}
        getRowClass={getRowClass}
        columnDefs={colDefs}
        theme={agGridTheme}
        animateRows={false}
        getRowId={(result) => (result.data.item.object_id || 0).toString()}
        onCellEditingStopped={onCellEditingStopped}
      />
    );
  },
);

export const ItemList = ({ search }: { search: string }) => {
  const deferredSearch = useDeferredValue(search, "");

  const itemsPromise = useMemo(
    () => fetchApi("/items").then((r) => r.json()) as Promise<Item[]>,
    [],
  );

  const locationsPromise = useMemo(
    () =>
      fetchApi("/locations").then((r) => r.json()) as Promise<ItemLocation[]>,
    [],
  );

  const [errorMessage, setErrorMessage] = useState("");

  return (
    <section className={itemClasses.listContainer}>
      {errorMessage && (
        <div className={itemClasses.errorMessage}>{errorMessage}</div>
      )}
      <Suspense fallback={<p>Loading ...</p>}>
        <ItemsListInner
          itemsPromise={itemsPromise}
          locationsPromise={locationsPromise}
          search={deferredSearch}
          setErrorMessage={setErrorMessage}
        />
      </Suspense>
    </section>
  );
};
